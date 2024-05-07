extern crate mlua;

use std::{ffi::OsString, io::{BufRead, BufReader}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::mpsc::channel, thread};

use mlua::{Error, Function, Lua, MultiValue, Table, Value};

fn script_dir<'lua>(lua: &'lua Lua, _args: MultiValue) -> mlua::Result<Value<'lua>> {
    let info = lua.inspect_stack(1)
        .ok_or_else(|| Error::runtime("Error retrieving stack information"))?;

    let source = info.source().source
        .ok_or_else(|| Error::runtime("Error getting source information from the stack"))?;

    if !source.starts_with("@") {
        return Ok(Value::Nil);
    }

    let source_path = PathBuf::from(source[1..].to_owned());
    let source_dir = source_path.parent();

    let source_dir_str_opt = source_dir
        .and_then(|d| d.to_str());

    match source_dir_str_opt {
        Some(s) => {
            if s.len() == 0 {
                Ok(Value::String(lua.create_string(".")?))
            } else {
                Ok(Value::String(lua.create_string(s)?))
            }
        },
        None => Ok(Value::Nil)
    }
}

enum ChildMessage {
    Stdout(String),
    StdoutDone,
    Stderr(String),
    StderrDone
}

fn exec_shell_command<'lua>(lua: &'lua Lua, args: Table<'lua>) -> mlua::Result<Table<'lua>> {
    let args_len_int = args.len()?;
    let args_len: usize = args_len_int.try_into()
        .map_err(|e| Error::runtime(format!("Invalid indices used for command args: {}", e)))?;

    let mut cmd_with_args: Vec<String> = Vec::with_capacity(args_len);
    cmd_with_args.resize(args_len, String::new());

    let mut cwd: Option<PathBuf> = None;
    let mut out_func: Option<Function> = None;
    let mut err_func: Option<Function> = None;

    for pair in args.pairs() {
        let (k, v): (Value, Value) = pair?;
        match k {
            Value::Integer(i) => {
                let idx: usize = i.try_into()
                    .map_err(|e| Error::runtime(format!("Invalid index used for command args: {}", e)))?;
                if idx > cmd_with_args.len() {
                    return Err(Error::runtime(format!("Invalid index used for command args: {}", idx)));
                }
                cmd_with_args[idx-1] = lua.unpack(v)?;
            },
            Value::String(s) => {
                let s_str = s.to_str().map_err(|e| Error::runtime(format!("Error reading lua string value: {}", e)))?;
                match s_str {
                    "cwd" => { cwd = Some(PathBuf::from(lua.unpack::<String>(v)?)); },
                    "out" => { out_func = Some(lua.unpack(v)?); },
                    "err" => { err_func = Some(lua.unpack(v)?); },
                    _ => { return Err(Error::runtime(format!("Unknown key in cmd input: {}", s.to_str().unwrap_or("<error reading value>")))); }
                };
            }
            _ => { return Err(Error::runtime(format!("Key type not allowed in cmd input: {}", k.type_name()))); }
        };
    }

    if cmd_with_args.len() < 1 {
        return Err(Error::runtime("No command given"));
    }
    let cmd_cmd = &cmd_with_args[0];
    let cmd_args = &cmd_with_args[1..];

    let mut cmd = Command::new(cmd_cmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.args(cmd_args);

    if let Some(d) = cwd {
        cmd.current_dir(d);
    }

    let child_res = cmd.spawn();

    match child_res {
        Err(e) => Err(Error::runtime(format!("Error executing command: {}", e))),
        Ok(mut child) => {
            let (tx, rx) = channel();

            let stdout_tx = tx.clone();
            let stdout = child.stdout.take().unwrap();
            let stdout_thread = thread::spawn(move || {
                let mut buf = String::with_capacity(256);
                let mut reader = BufReader::new(stdout);
                loop {
                    let res = reader.read_line(&mut buf);
                    match res {
                        Ok(bytes_read) => {
                            if bytes_read == 0 { break; }
                            let out = buf.clone();
                            buf.clear();
                            stdout_tx.send(ChildMessage::Stdout(out)).unwrap();
                        }
                        Err(_) => { break; }
                    }
                }
                stdout_tx.send(ChildMessage::StdoutDone).unwrap();
            });

            let stderr_tx = tx.clone();
            let stderr = child.stderr.take().unwrap();
            let stderr_thread = thread::spawn(move || {
                let mut buf = String::with_capacity(256);
                let mut reader = BufReader::new(stderr);
                loop {
                    let res = reader.read_line(&mut buf);
                    match res {
                        Ok(bytes_read) => {
                            if bytes_read == 0 { break; }
                            let err = buf.clone();
                            buf.clear();
                            stderr_tx.send(ChildMessage::Stderr(err)).unwrap();
                        }
                        Err(_) => { break; }
                    }
                }
                stderr_tx.send(ChildMessage::StderrDone).unwrap();
            });

            let mut stdout_buf = String::new();
            let mut stdout_done = false;

            let mut stderr_buf = String::new();
            let mut stderr_done = false;

            while !stdout_done || !stderr_done {
                let msg = rx.recv().unwrap();

                match msg {
                    ChildMessage::Stdout(out) => {
                        stdout_buf.push_str(out.as_str());
                        if let Some(out_fn) = &out_func {
                            out_fn.call(out)?;
                        }
                    },
                    ChildMessage::StdoutDone => { stdout_done = true; },
                    ChildMessage::Stderr(err) => {
                        stderr_buf.push_str(err.as_str());
                        if let Some(err_fn) = &err_func {
                            err_fn.call(err)?;
                        }
                    },
                    ChildMessage::StderrDone => { stderr_done = true; }
                }
            }

            stdout_thread.join().unwrap();
            stderr_thread.join().unwrap();

            let status_res = child.wait();
            let status = match status_res {
                Ok(status) => lua.pack(status.code())?,
                Err(e) => { return Err(Error::runtime(format!("{}", e))); }
            };

            let stdout = lua.create_string(stdout_buf)?;
            let stderr = lua.create_string(stderr_buf)?;

            let result = lua.create_table()?;
            result.set("stdout", stdout)?;
            result.set("stderr", stderr)?;
            result.set("status", status)?;
            Ok(result)
        }
    }
}

pub fn create_lua_env(module_root_path: &Path) -> mlua::Result<Lua> {
    let lua = unsafe { Lua::unsafe_new() };

    let if_else_func: Function = lua.load(r#"
        function (cond, true_val, false_val)
            if cond then
                return true_val
            else
                return false_val
            end
        end
    "#).eval()?;
    lua.globals().set("if_else", if_else_func)?;

    let cmd_func = lua.create_function(exec_shell_command)?;
    lua.globals().set("cmd", cmd_func)?;

    let script_dir_func = lua.create_function(script_dir)?;
    lua.globals().set("script_dir", script_dir_func)?;

    {
        let mut module_search_path = OsString::new();
        module_search_path.push(module_root_path.as_os_str());
        module_search_path.push("/?.lua;");
        module_search_path.push(module_root_path.as_os_str());
        module_search_path.push("/?/init.lua");
    
        let package_global: Table = lua.globals().get("package")?;
        package_global.set("path", module_search_path.to_str())?;
    }

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_command() {
        let lua_env = create_lua_env(Path::new(".")).unwrap();
        let chunk = lua_env.load("cmd({\"echo\", \"hi!\"})");

        let result: Table = chunk.eval().unwrap();
        assert_eq!(result.get::<_, i32>("status").unwrap(), 0);
        assert_eq!(result.get::<_, String>("stdout").unwrap(), "hi!\n");
    }

    #[test]
    fn test_if_else() {
        let lua_env = create_lua_env(Path::new(".")).unwrap();
        let chunk = lua_env.load(r#"if_else(5 < 3, "yah", "nah")"#);
        let result: String = chunk.eval().unwrap();
        assert_eq!(result, "nah");
    }
}
