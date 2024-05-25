use std::{fs::File, io::{Read, Write}, path::Path};

use mlua::{Lua, UserData};

use crate::project_def::validate::validate_table_is_sequence;

pub struct JsonLib;

impl UserData for JsonLib {
    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("load", json_load);
        methods.add_function("loads", json_loads);
        methods.add_function("dump", json_dump);
        methods.add_function("dumps", json_dumps);
    }
}

fn json_load<'lua>(lua: &'lua Lua, path: String) -> mlua::Result<mlua::Value<'lua>> {
    let mut f = File::open(Path::new(path.as_str()))
        .map_err(|e| mlua::Error::runtime(format!("Error reading file {}: {}", path, e)))?;

    let mut buf = String::new();
    f.read_to_string(&mut buf)
        .map_err(|e| mlua::Error::runtime(format!("Error reading file {}: {}", path, e)))?;

    json_loads(lua, buf)
}

fn json_loads<'lua>(lua: &'lua Lua, val: String) -> mlua::Result<mlua::Value<'lua>> {
    let parsed: serde_json::Value = val
        .parse()
        .map_err(|e| mlua::Error::runtime(format!("Error parsing json: {}", e)))?;

    json_to_lua(lua, parsed)
}

fn json_dump<'lua>(lua: &'lua Lua, args: (String, mlua::Value<'lua>)) -> mlua::Result<()> {
    let (path, lua_val) = args;
    let json_str = json_dumps(lua, lua_val)?;
    
    let mut f = File::create(Path::new(path.as_str()))
        .map_err(|e| mlua::Error::runtime(format!("Error opening file {}: {}", path, e)))?;

    f.write_all(json_str.as_bytes())
        .map_err(|e| mlua::Error::runtime(format!("Error writing to file {}: {}", path, e)))?;

    Ok(())
}

fn json_dumps<'lua>(lua: &'lua Lua, val: mlua::Value) -> mlua::Result<String> {
    let json_val = lua_to_json(lua, val)?;
    Ok(json_val.to_string())
}

fn json_to_lua<'lua>(
    lua: &'lua Lua,
    json_val: serde_json::Value,
) -> mlua::Result<mlua::Value<'lua>> {
    match json_val {
        serde_json::Value::Null => Ok(mlua::Value::Nil),
        serde_json::Value::Bool(b) => Ok(mlua::Value::Boolean(b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(|i| mlua::Value::Integer(i))
            .or_else(|| n.as_f64().map(|f| mlua::Value::Number(f)))
            .ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Unable to convert json number {} to a float or int",
                    n
                ))
            }),
        serde_json::Value::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table_with_capacity(arr.len(), 0)?;
            for v in arr {
                table.push(json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table_with_capacity(0, obj.len())?;
            for (k, v) in obj {
                table.set(k, json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
    }
}

fn lua_to_json<'lua>(
    lua: &'lua Lua,
    lua_val: mlua::Value<'lua>,
) -> mlua::Result<serde_json::Value> {
    match lua_val {
        mlua::Value::Nil => Ok(serde_json::Value::Null),
        mlua::Value::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        mlua::Value::Integer(i) => Ok(serde_json::Value::Number(i.into())),
        mlua::Value::Number(n) => match serde_json::Number::from_f64(n) {
            Some(json_n) => Ok(serde_json::Value::Number(json_n)),
            None => Err(mlua::Error::runtime(format!(
                "Unable to convert float value {} to a json number",
                n
            ))),
        },
        mlua::Value::String(s) => Ok(serde_json::Value::String(s.to_str()?.to_owned())),
        mlua::Value::Table(t) => match validate_table_is_sequence(&t, None, &mut Vec::new()) {
            Ok(_) => {
                let mut arr: Vec<serde_json::Value> = Vec::with_capacity(t.len()? as usize);
                for v_res in t.sequence_values() {
                    let v: mlua::Value = v_res?;
                    arr.push(lua_to_json(lua, v)?);
                }
                Ok(serde_json::Value::Array(arr))
            }
            Err(_) => {
                let mut map: serde_json::map::Map<String, serde_json::Value> =
                    serde_json::map::Map::new();
                for pair in t.pairs() {
                    let (k, v): (String, mlua::Value) = pair?;
                    map.insert(k, lua_to_json(lua, v)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        },
        mlua::Value::UserData(_) => Err(mlua::Error::runtime(
            "Cannot convert a userdata to a json value",
        )),
        mlua::Value::Function(_) => Err(mlua::Error::runtime(
            "Cannot convert a function to a json value",
        )),
        mlua::Value::LightUserData(_) => Err(mlua::Error::runtime(
            "Cannot convert a lightuserdata to a json value",
        )),
        mlua::Value::Thread(_) => Err(mlua::Error::runtime(
            "Cannot convert a thread to a json value",
        )),
        mlua::Value::Error(_) => Err(mlua::Error::runtime(
            "Cannot convert an error object to a json value",
        )),
    }
}
