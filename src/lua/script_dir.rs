// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::path::PathBuf;

use mlua::{Error, Lua, MultiValue, UserData, Value};

pub struct ScriptDirLib;

impl UserData for ScriptDirLib {
    fn add_methods<M: mlua::prelude::LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("script_dir", script_dir);
    }
}

fn script_dir(lua: &Lua, _args: MultiValue) -> mlua::Result<Value> {
    let source = lua
        .inspect_stack(2, |d| {
            d
                .source()
                .source
                .map(String::from)
                .ok_or_else(|| Error::runtime("Error getting source information from the stack"))
        })
        .unwrap_or_else(|| Err(Error::runtime("Error retrieving lua environment debug info")))?;

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
