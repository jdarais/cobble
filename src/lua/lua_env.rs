use std::ffi::OsString;
use std::path::Path;

use mlua::{Lua, Table};

use crate::lua::cmd::CmdLib;
use crate::lua::fs::FsLib;
use crate::lua::script_dir::ScriptDirLib;
use crate::lua::toml::TomlLib;

pub fn create_lua_env(workspace_dir: &Path) -> mlua::Result<Lua> {
    let lua = unsafe { Lua::unsafe_new() };

    let workspace_table = lua.create_table()?;
    workspace_table.set("dir", workspace_dir.to_str())?;
    lua.globals().set("WORKSPACE", workspace_table)?;

    let on_scope_exit_source = include_bytes!("on_scope_exit.lua");
    lua.load(&on_scope_exit_source[..]).exec()?;

    let version_source = include_bytes!("version.lua");
    lua.load(&version_source[..]).exec()?;

    let table_util_source = include_bytes!("table_util.lua");
    lua.load(&table_util_source[..]).exec()?;

    let cmd_lib = lua.create_userdata(CmdLib)?;
    let cmd_source = include_bytes!("cmd.lua");
    lua.load(&cmd_source[..]).call(cmd_lib)?;

    let fslib = lua.create_userdata(FsLib)?;
    let fs_source = include_bytes!("fs.lua");
    lua.load(&fs_source[..]).call(fslib)?;

    let script_dir_lib = lua.create_userdata(ScriptDirLib)?;
    let script_dir_source = include_bytes!("script_dir.lua");
    lua.load(&script_dir_source[..]).call(script_dir_lib)?;

    let iter_source = include_bytes!("iter.lua");
    lua.load(&iter_source[..]).exec()?;

    let maybe_source = include_bytes!("maybe.lua");
    lua.load(&maybe_source[..]).exec()?;

    let toml_lib = lua.create_userdata(TomlLib)?;
    let toml_source = include_bytes!("toml.lua");
    lua.load(&toml_source[..]).call(toml_lib)?;

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
}
