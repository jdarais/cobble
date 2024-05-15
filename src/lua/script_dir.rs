use std::path::PathBuf;

use mlua::{Error, Lua, MultiValue, Value};

pub fn script_dir<'lua>(lua: &'lua Lua, _args: MultiValue) -> mlua::Result<Value<'lua>> {
    let info = lua
        .inspect_stack(1)
        .ok_or_else(|| Error::runtime("Error retrieving stack information"))?;

    let source = info
        .source()
        .source
        .ok_or_else(|| Error::runtime("Error getting source information from the stack"))?;

    if !source.starts_with("@") {
        return Ok(Value::Nil);
    }

    let source_path = PathBuf::from(source[1..].to_owned());
    let source_dir = source_path.parent();

    let source_dir_str_opt = source_dir.and_then(|d| d.to_str());

    match source_dir_str_opt {
        Some(s) => {
            if s.len() == 0 {
                Ok(Value::String(lua.create_string(".")?))
            } else {
                Ok(Value::String(lua.create_string(s)?))
            }
        }
        None => Ok(Value::Nil),
    }
}
