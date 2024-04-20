extern crate mlua;

use std::{ffi::OsString, path::Path, process::Command};

fn exec_shell_command(lua: &mlua::Lua, cmd_with_args: Vec<String>) -> mlua::Result<mlua::Table> {
    if cmd_with_args.len() < 1 {
        return Err(mlua::Error::RuntimeError(String::from("No command given")));
    }
    let cmd = &cmd_with_args[0];
    let args = &cmd_with_args[1..];

    let output_res = Command::new(cmd)
        .args(args)
        .output();

    match output_res {
        Err(e) => Err(mlua::Error::RuntimeError(format!("Error executing command: {}", e))),
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
    use std::path::PathBuf;

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
