extern crate mlua;

use std::process::Command;

fn exec_shell_command(_: &mlua::Lua, cmd_with_args: Vec<String>) -> Result<(i32, String), mlua::Error> {
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
            let stdout_res = String::from_utf8(output.stdout);
            match stdout_res {
                Err(e) => Err(mlua::Error::RuntimeError(format!("Unable to convert command output to utf-8: {:?}", e))),
                Ok(stdout) => Ok((output.status.code().unwrap_or(-1), stdout))
            }
        }
    }
}

pub fn create_lua_env() -> Result<mlua::Lua, mlua::Error> {
    let lua = unsafe { mlua::Lua::unsafe_new() };

    let cmd_func = lua.create_function(exec_shell_command)?;
    lua.globals().set("cmd", cmd_func)?;

    let cobble_table = lua.create_table()?;

    let build_envs = lua.create_table()?;
    cobble_table.set("build_envs", build_envs)?;
    
    lua.globals().set("cobble", cobble_table)?;

    Ok(lua)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_command() {
        let lua_env = create_lua_env().unwrap();
        let chunk = lua_env.load(String::from("cmd({\"echo\", \"hi!\"})"));

        let (status, stdout): (i32, String) = chunk.eval().unwrap();
        assert_eq!(status, 0);
        assert_eq!(stdout, "hi!\n");
    }
}
