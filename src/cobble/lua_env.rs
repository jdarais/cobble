extern crate mlua;

use mlua::Lua;
use std::process::Command;

fn exec_shell_command(_: &Lua, cmd_with_args: Vec<String>) -> Result<(i32, String), mlua::Error> {
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
                Err(e) => Err(mlua::Error::RuntimeError(String::from("Unable to convert command output to utf-8"))),
                Ok(stdout) => Ok((output.status.code().unwrap_or(-1), stdout))
            }
        }
    }
}


pub fn create_lua_env() -> Result<Lua, mlua::Error> {
    let lua_env = Lua::new();

    let func = lua_env.create_function(exec_shell_command)?;
    lua_env.globals().set("cmd", func)?;

    // TODO: Load plugins

    Ok(lua_env)
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
