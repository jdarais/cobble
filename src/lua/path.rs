use std::path::Path;

use mlua::Lua;

pub fn is_dir<'lua>(_lua: &'lua Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_dir())
}

pub fn is_file<'lua>(_lua: &'lua Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_file())
}