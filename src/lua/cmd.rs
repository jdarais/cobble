use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;

use mlua::{Error, Function, Lua, Table, Value};

enum ChildMessage {
    Stdout(String),
    StdoutDone,
    Stderr(String),
    StderrDone,
}

pub fn exec_shell_command<'lua>(lua: &'lua Lua, args: Table<'lua>) -> mlua::Result<Table<'lua>> {
    let args_len_int = args.len()?;
    let args_len: usize = args_len_int
        .try_into()
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
                let idx: usize = i.try_into().map_err(|e| {
                    Error::runtime(format!("Invalid index used for command args: {}", e))
                })?;
                if idx > cmd_with_args.len() {
                    return Err(Error::runtime(format!(
                        "Invalid index used for command args: {}",
                        idx
                    )));
                }
                cmd_with_args[idx - 1] = lua.unpack(v)?;
            }
            Value::String(s) => {
                let s_str = s.to_str().map_err(|e| {
                    Error::runtime(format!("Error reading lua string value: {}", e))
                })?;
                match s_str {
                    "cwd" => {
                        cwd = Some(PathBuf::from(lua.unpack::<String>(v)?));
                    }
                    "out" => {
                        out_func = Some(lua.unpack(v)?);
                    }
                    "err" => {
                        err_func = Some(lua.unpack(v)?);
                    }
                    _ => {
                        return Err(Error::runtime(format!(
                            "Unknown key in cmd input: {}",
                            s.to_str().unwrap_or("<error reading value>")
                        )));
                    }
                };
            }
            _ => {
                return Err(Error::runtime(format!(
                    "Key type not allowed in cmd input: {}",
                    k.type_name()
                )));
            }
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
                            if bytes_read == 0 {
                                break;
                            }
                            let out = buf.clone();
                            buf.clear();
                            stdout_tx.send(ChildMessage::Stdout(out)).unwrap();
                        }
                        Err(_) => {
                            break;
                        }
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
                            if bytes_read == 0 {
                                break;
                            }
                            let err = buf.clone();
                            buf.clear();
                            stderr_tx.send(ChildMessage::Stderr(err)).unwrap();
                        }
                        Err(_) => {
                            break;
                        }
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
                    }
                    ChildMessage::StdoutDone => {
                        stdout_done = true;
                    }
                    ChildMessage::Stderr(err) => {
                        stderr_buf.push_str(err.as_str());
                        if let Some(err_fn) = &err_func {
                            err_fn.call(err)?;
                        }
                    }
                    ChildMessage::StderrDone => {
                        stderr_done = true;
                    }
                }
            }

            stdout_thread.join().unwrap();
            stderr_thread.join().unwrap();

            let status_res = child.wait();
            let status = match status_res {
                Ok(status) => lua.pack(status.code())?,
                Err(e) => {
                    return Err(Error::runtime(format!("{}", e)));
                }
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
