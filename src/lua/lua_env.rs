extern crate mlua;

use std::{ffi::OsString, path::{Path, PathBuf}, process::Command};

fn exec_shell_command<'lua>(lua: &'lua mlua::Lua, args: mlua::Table<'lua>) -> mlua::Result<mlua::Table<'lua>> {
    let args_len_int = args.len()?;
    let args_len: usize = args_len_int.try_into()
        .map_err(|e| mlua::Error::runtime(format!("Invalid indices used for command args: {}", e)))?;

    let mut cmd_with_args: Vec<String> = Vec::with_capacity(args_len);
    cmd_with_args.resize(args_len, String::new());

    let mut cwd: Option<PathBuf> = None;

    for pair in args.pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::Integer(i) => {
                let idx: usize = i.try_into()
                    .map_err(|e| mlua::Error::runtime(format!("Invalid index used for command args: {}", e)))?;
                if idx > cmd_with_args.len() {
                    return Err(mlua::Error::runtime(format!("Invalid index used for command args: {}", idx)));
                }
                cmd_with_args[idx-1] = lua.unpack(v)?;
            },
            mlua::Value::String(s) => {
                let s_str = s.to_str().map_err(|e| mlua::Error::runtime(format!("Error reading lua string value: {}", e)))?;
                match s_str {
                    "cwd" => {
                        let path_str: String = lua.unpack(v)?;
                        cwd = Some(PathBuf::from(path_str));
                    },
                    _ => { return Err(mlua::Error::runtime(format!("Unknown key in cmd input: {}", s.to_str().unwrap_or("<error reading value>")))); }
                };
            }
            _ => { return Err(mlua::Error::runtime(format!("Key type not allowed in cmd input: {}", k.type_name()))); }
        };
    }

    println!("{:?}", &cmd_with_args);
    println!("{:?}", cwd.as_ref().map(|p| p.display().to_string()));

    if cmd_with_args.len() < 1 {
        return Err(mlua::Error::runtime("No command given"));
    }
    let cmd_cmd = &cmd_with_args[0];
    let cmd_args = &cmd_with_args[1..];

    let mut cmd = Command::new(cmd_cmd);
    cmd.args(cmd_args);

    if let Some(d) = cwd {
        cmd.current_dir(d);
    }

    let output_res = cmd.output();

    match output_res {
        Err(e) => Err(mlua::Error::runtime(format!("Error executing command: {}", e))),
        Ok(output) => {
            let stdout = lua.create_string(output.stdout)?;
            let stderr = lua.create_string(output.stderr)?;

            let result = lua.create_table()?;
            result.set("stdout", stdout)?;
            result.set("stderr", stderr)?;
            result.set("status", output.status.code())?;
            Ok(result)
        }
    }
}

pub fn create_lua_env(module_root_path: &Path) -> mlua::Result<mlua::Lua> {
    let lua = unsafe { mlua::Lua::unsafe_new() };

    let cmd_func = lua.create_function(exec_shell_command)?;
    lua.globals().set("cmd", cmd_func)?;

    {
        let mut module_search_path = OsString::new();
        module_search_path.push(module_root_path.as_os_str());
        module_search_path.push("/?.lua;");
        module_search_path.push(module_root_path.as_os_str());
        module_search_path.push("/?/init.lua");
    
        let package_global: mlua::Table = lua.globals().get("package")?;
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
        let chunk = lua_env.load(String::from("cmd({\"echo\", \"hi!\"})"));

        let result: mlua::Table = chunk.eval().unwrap();
        assert_eq!(result.get::<_, i32>("status").unwrap(), 0);
        assert_eq!(result.get::<_, String>("stdout").unwrap(), "hi!\n");
    }
}
