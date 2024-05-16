use std::ffi::OsString;
use std::path::Path;

use mlua::{Lua, Table};

use crate::lua::cmd::exec_shell_command;
use crate::lua::glob::glob_files;
use crate::lua::path::{is_dir, is_file};
use crate::lua::script_dir::script_dir;
use crate::lua::toml::toml_loads;

pub fn create_lua_env(workspace_dir: &Path) -> mlua::Result<Lua> {
    let lua = unsafe { Lua::unsafe_new() };

    let workspace_table = lua.create_table()?;
    workspace_table.set("dir", workspace_dir.to_str())?;
    lua.globals().set("WORKSPACE", workspace_table)?;

    let if_else_source = include_bytes!("if_else.lua");
    lua.load(&if_else_source[..]).exec()?;

    let cmd_func = lua.create_function(exec_shell_command)?;
    lua.globals().set("cmd", cmd_func)?;

    let glob_func = lua.create_function(glob_files)?;
    lua.globals().set("glob", glob_func)?;

    let is_dir_func = lua.create_function(is_dir)?;
    lua.globals().set("is_dir", is_dir_func)?;

    let is_file_func = lua.create_function(is_file)?;
    lua.globals().set("is_file", is_file_func)?;

    let script_dir_func = lua.create_function(script_dir)?;
    lua.globals().set("script_dir", script_dir_func)?;

    let iter_source = include_bytes!("iter.lua");
    lua.load(&iter_source[..]).exec()?;

    let maybe_source = include_bytes!("maybe.lua");
    lua.load(&maybe_source[..]).exec()?;

    let toml_table = lua.create_table()?;
    let toml_loads_func = lua.create_function(toml_loads)?;
    toml_table.set("loads", toml_loads_func)?;
    lua.globals().set("toml", toml_table)?;

    {
        let mut module_search_path = OsString::new();
        module_search_path.push(workspace_dir.as_os_str());
        module_search_path.push("/?.lua;");
        module_search_path.push(workspace_dir.as_os_str());
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
