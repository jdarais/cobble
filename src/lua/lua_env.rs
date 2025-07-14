// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Path;

use mlua::{Lua, Table};

use crate::lua::cmd::CmdLib;
use crate::lua::fs::FsLib;
use crate::lua::json::JsonLib;
use crate::lua::path::PathLib;
use crate::lua::script_dir::ScriptDirLib;
use crate::lua::toml::TomlLib;

pub const COBBLE_JOB_INTERACTIVE_ENABLED: &str = "COBBLE_JOB_INTERACTIVE_ENABLED";

pub fn create_lua_env(workspace_dir: &Path, modules_dir: &Path) -> mlua::Result<Lua> {
    let lua = unsafe { Lua::unsafe_new() };
    let preload_table: mlua::Table = lua
        .globals()
        .get::<_, mlua::Table>("package")?
        .get("preload")?;

    let workspace_table = lua.create_table()?;
    workspace_table.set("dir", workspace_dir.to_str())?;
    lua.globals().set("WORKSPACE", workspace_table)?;

    let platform_table = lua.create_table()?;
    platform_table.set("arch", std::env::consts::ARCH)?;
    platform_table.set("os_family", std::env::consts::FAMILY)?;
    platform_table.set("os", std::env::consts::OS)?;
    lua.globals().set("PLATFORM", platform_table)?;

    let cmd_lib = lua.create_userdata(CmdLib)?;
    let cmd_source = include_bytes!("cmd.lua");
    let cmd_loader = lua.load(&cmd_source[..]).into_function()?.bind(cmd_lib)?;
    preload_table.set("cmd", cmd_loader)?;

    let fs_lib = lua.create_userdata(FsLib)?;
    let fs_source = include_bytes!("fs.lua");
    let fs_loader = lua.load(&fs_source[..]).into_function()?.bind(fs_lib)?;
    preload_table.set("fs", fs_loader)?;

    let path_lib = lua.create_userdata(PathLib)?;
    let path_source = include_bytes!("path.lua");
    let path_loader = lua.load(&path_source[..]).into_function()?.bind(path_lib)?;
    preload_table.set("path", path_loader)?;

    let iter_source = include_bytes!("iter.lua");
    let iter_loader = lua.load(&iter_source[..]).into_function()?;
    preload_table.set("iter", iter_loader)?;

    let json_lib = lua.create_userdata(JsonLib)?;
    let json_source = include_bytes!("json.lua");
    let json_loader = lua.load(&json_source[..]).into_function()?.bind(json_lib)?;
    preload_table.set("json", json_loader)?;

    let maybe_source = include_bytes!("maybe.lua");
    let maybe_loader = lua.load(&maybe_source[..]).into_function()?;
    preload_table.set("maybe", maybe_loader)?;

    let scope_source = include_bytes!("scope.lua");
    let scope_loader = lua.load(&scope_source[..]).into_function()?;
    preload_table.set("scope", scope_loader)?;

    let script_dir_lib = lua.create_userdata(ScriptDirLib)?;
    let script_dir_source = include_bytes!("script_dir.lua");
    let script_dir_loader = lua
        .load(&script_dir_source[..])
        .into_function()?
        .bind(script_dir_lib)?;
    preload_table.set("script_dir", script_dir_loader)?;

    let toml_lib = lua.create_userdata(TomlLib)?;
    let toml_source = include_bytes!("toml.lua");
    let toml_loader = lua.load(&toml_source[..]).into_function()?.bind(toml_lib)?;
    preload_table.set("toml", toml_loader)?;

    let version_source = include_bytes!("version.lua");
    let version_loader = lua.load(&version_source[..]).into_function()?;
    preload_table.set("version", version_loader)?;

    let tblext_source = include_bytes!("tblext.lua");
    let tblext_loader = lua.load(&tblext_source[..]).into_function()?;
    preload_table.set("tblext", tblext_loader)?;

    let ordered_map_source = include_bytes!("ordered_map.lua");
    let ordered_map_loader = lua.load(&ordered_map_source[..]).into_function()?;
    preload_table.set("ordered_map", ordered_map_loader)?;

    let workspace_dir_full_path = dunce::canonicalize(workspace_dir)
        .map_err(|e| mlua::Error::runtime(format!("Error determining workspace directory from {}: {}", workspace_dir.display(), e)))?;

    let modules_dir_full_path: Cow<Path> = dunce::canonicalize(workspace_dir.join(modules_dir))
        .map(|p| Cow::Owned(p))
        .unwrap_or(Cow::Borrowed(workspace_dir));

    if !modules_dir_full_path.starts_with(&workspace_dir_full_path) {
        return Err(mlua::Error::runtime(format!("Modules directory must be inside the workspace. (Got {})", modules_dir_full_path.display())));
    }

    {
        let mut module_search_path = OsString::new();
        module_search_path.push(modules_dir_full_path.as_os_str());
        module_search_path.push(std::path::MAIN_SEPARATOR_STR);
        module_search_path.push("?.lua;");
        module_search_path.push(modules_dir_full_path.as_os_str());
        module_search_path.push(std::path::MAIN_SEPARATOR_STR);
        module_search_path.push("?");
        module_search_path.push(std::path::MAIN_SEPARATOR_STR);
        module_search_path.push("init.lua");

        let package_global: Table = lua.globals().get("package")?;
        package_global.set("path", module_search_path.to_str())?;
    }

    drop(preload_table);
    Ok(lua)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_shell_command() {
        let lua_env = create_lua_env(Path::new(".")).unwrap();
        let chunk = lua_env.load(r#"require("cmd")({"echo", "hi!"})"#);

        let result: Table = chunk.eval().unwrap();
        assert_eq!(result.get::<_, i32>("status").unwrap(), 0);
        assert_eq!(result.get::<_, String>("stdout").unwrap(), "hi!\n");
    }
}
