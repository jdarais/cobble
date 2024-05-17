use std::{fs::File, io::Read, path::Path};

use mlua::{Lua, UserData};

pub struct TomlLib;

impl UserData for TomlLib {
    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("loads", toml_loads);
        methods.add_function("read", toml_read);
    }
}

fn toml_read<'lua>(lua: &'lua Lua, path: String) -> mlua::Result<mlua::Value<'lua>> {
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
