// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{fs::File, io::{Read, Write}, path::Path};

use mlua::{Lua, UserData};

use crate::project_def::validate::validate_table_is_sequence;

pub struct TomlLib;

impl UserData for TomlLib {
    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("loads", toml_loads);
        methods.add_function("load", toml_load);
        methods.add_function("dumps", toml_dumps);
        methods.add_function("dump", toml_dump);
    }
}

fn toml_load<'lua>(lua: &'lua Lua, path: String) -> mlua::Result<mlua::Value<'lua>> {
    let mut f = File::open(Path::new(path.as_str()))
        .map_err(|e| mlua::Error::runtime(format!("Error reading file {}: {}", path, e)))?;

    let mut buf = String::new();
    f.read_to_string(&mut buf)
        .map_err(|e| mlua::Error::runtime(format!("Error reading file {}: {}", path, e)))?;

    toml_loads(lua, buf)
}

fn toml_loads<'lua>(lua: &'lua Lua, toml_str: String) -> mlua::Result<mlua::Value<'lua>> {
    let toml_tbl = toml_str
        .parse::<toml::Table>()
        .map_err(|e| mlua::Error::runtime(format!("Error parsing toml: {}", e)))?;

    toml_to_lua(lua, toml::Value::Table(toml_tbl))
}

fn toml_dump<'lua>(lua: &'lua Lua, args: (String, mlua::Table<'lua>)) -> mlua::Result<()> {
    let (path, table) = args;
    let toml_str = toml_dumps(lua, table)?;
    
    let mut f = File::create(Path::new(path.as_str()))
        .map_err(|e| mlua::Error::runtime(format!("Error opening file {}: {}", path, e)))?;

    f.write_all(toml_str.as_bytes())
        .map_err(|e| mlua::Error::runtime(format!("Error writing to file {}: {}", path, e)))?;

    Ok(())
}

fn toml_dumps<'lua>(lua: &'lua Lua, table: mlua::Table<'lua>) -> mlua::Result<mlua::String<'lua>> {
    let toml = lua_to_toml(lua, mlua::Value::Table(table))?;
    let toml_str = toml.to_string();
    let toml_lua_str = lua.create_string(toml_str)?;
    Ok(toml_lua_str)
}

struct DateTimeUserData(toml::value::Datetime);

impl UserData for DateTimeUserData {
    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |lua, this, _args: mlua::MultiValue| {
            let formatted = format!("{}", this.0);
            let lua_string = lua.create_string(formatted)?;
            Ok(mlua::Value::String(lua_string))
        });
    }
}

fn toml_to_lua<'lua>(lua: &'lua Lua, toml_val: toml::Value) -> mlua::Result<mlua::Value> {
    match toml_val {
        toml::Value::Array(arr) => {
            let tbl = lua.create_table()?;
            for val in arr {
                tbl.push(toml_to_lua(lua, val)?)?;
            }
            Ok(mlua::Value::Table(tbl))
        }
        toml::Value::Table(toml_tbl) => {
            let tbl = lua.create_table()?;
            for (k, v) in toml_tbl {
                tbl.set(k, toml_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(tbl))
        }
        toml::Value::String(s) => {
            let lua_str = lua.create_string(s)?;
            Ok(mlua::Value::String(lua_str))
        }
        toml::Value::Integer(i) => Ok(mlua::Value::Integer(i)),
        toml::Value::Float(f) => Ok(mlua::Value::Number(f)),
        toml::Value::Boolean(b) => Ok(mlua::Value::Boolean(b)),
        toml::Value::Datetime(d) => {
            let userdata = lua.create_any_userdata(DateTimeUserData(d))?;
            Ok(mlua::Value::UserData(userdata))
        }
    }
}

fn lua_to_toml<'lua>(lua: &'lua mlua::Lua, lua_val: mlua::Value<'lua>) -> mlua::Result<toml::Value> {
    match lua_val {
        mlua::Value::Boolean(b) => Ok(toml::Value::Boolean(b)),
        mlua::Value::Number(n) => Ok(toml::Value::Float(n)),
        mlua::Value::Integer(i) => Ok(toml::Value::Integer(i)),
        mlua::Value::String(s) => {
            let toml_str = s.to_str()?;
            Ok(toml::Value::String(toml_str.to_owned()))
        }
        mlua::Value::Table(t) => match validate_table_is_sequence(&t, None, &mut Vec::new()) {
            Ok(_) => {
                let mut arr: Vec<toml::Value> = Vec::with_capacity(t.len()? as usize);
                for v_res in t.sequence_values() {
                    let v: mlua::Value = v_res?;
                    arr.push(lua_to_toml(lua, v)?);
                }
                Ok(toml::Value::Array(arr))
            }
            Err(_) => {
                let mut map: toml::map::Map<String, toml::Value> = toml::map::Map::new();
                for pair in t.pairs() {
                    let (k, v): (String, mlua::Value) = pair?;
                    map.insert(k, lua_to_toml(lua, v)?);
                }
                Ok(toml::Value::Table(map))
            }
        }
        mlua::Value::UserData(d) => {
            if let Ok(datetime) = d.borrow::<DateTimeUserData>() {
                Ok(toml::Value::Datetime(datetime.0.clone()))
            } else {
                Err(mlua::Error::runtime("Cannot convert non-DateTime userdata to a toml value"))
            }
        }
        mlua::Value::Nil => Err(mlua::Error::runtime("Cannot convert nil to a toml value")),
        mlua::Value::Function(_) => Err(mlua::Error::runtime("Cannot convert a function to a toml value")),
        mlua::Value::LightUserData(_) => Err(mlua::Error::runtime("Cannot convert a lightuserdata to a toml value")),
        mlua::Value::Thread(_) => Err(mlua::Error::runtime("Cannot convert a thread to a toml value")),
        mlua::Value::Error(_) => Err(mlua::Error::runtime("Cannot convert an error object to a toml value"))
    }
}
