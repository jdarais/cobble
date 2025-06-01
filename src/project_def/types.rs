// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

use crate::project_def::validate::validate_table_is_sequence;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum StringOrInt {
    String(String),
    Int(i64),
}

impl fmt::Display for StringOrInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringOrInt::String(s) => write!(f, "\"{}\"", s),
            StringOrInt::Int(i) => write!(f, "{}", i),
        }
    }
}

impl<'lua> mlua::FromLua<'lua> for StringOrInt {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => Ok(StringOrInt::String(String::from(s.to_str()?))),
            mlua::Value::Integer(i) => Ok(StringOrInt::Int(i)),
            _ => Err(mlua::Error::runtime(format!(
                "Expected a string or integer, but got a {}: {:?}",
                value.type_name(),
                value
            ))),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum MapOrArray<T> {
    Map(HashMap<String, T>),
    Array(Vec<T>),
}

impl<T> MapOrArray<T> {
    pub fn len(&self) -> usize {
        match self {
            Self::Map(m) => m.len(),
            Self::Array(arr) => arr.len(),
        }
    }
}

impl <T> Default for MapOrArray<T> {
    fn default() -> Self {
        MapOrArray::Array(Vec::new())
    }
}

impl<T> From<MapOrArray<T>> for HashMap<StringOrInt, T> {
    fn from(value: MapOrArray<T>) -> Self {
        match value {
            MapOrArray::Map(m) => m
                .into_iter()
                .map(|(k, v)| (StringOrInt::String(k), v))
                .collect(),
            MapOrArray::Array(arr) => arr
                .into_iter()
                .enumerate()
                .map(|(i, v)| (StringOrInt::Int(i as i64), v))
                .collect(),
        }
    }
}

impl <'lua, T: mlua::FromLua<'lua>> mlua::FromLua<'lua> for MapOrArray<T> {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let is_sequence = validate_table_is_sequence(&tbl, None, &mut Vec::new()).is_ok();
                if is_sequence {
                    let mut arr: Vec<T> = Vec::new();
                    for val_res in tbl.clone().sequence_values() {
                        let val: T = val_res?;
                        arr.push(val);
                    }
                    Ok(MapOrArray::Array(arr))
                } else {
                    let mut map: HashMap<String, T> = HashMap::new();
                    for pairs_res in tbl.clone().pairs() {
                        let (k, v): (String, T) = pairs_res?;
                        map.insert(k, v);
                    }
                    Ok(MapOrArray::Map(map))
                }
            },
            _ => Err(mlua::Error::runtime(format!(
                "Expected a string or integer, but got a {}: {:?}",
                value.type_name(),
                value
            ))),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum TaskVar {
    Table(HashMap<String, TaskVar>),
    List(Vec<TaskVar>),
    String(String),
}

impl fmt::Display for TaskVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskVar::Table(t) => {
                f.write_str("{")?;
                for (i, (k, v)) in t.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                f.write_str("}")
            }
            TaskVar::List(l) => {
                f.write_str("[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                f.write_str("]")
            }
            TaskVar::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

impl<'lua> mlua::FromLua<'lua> for TaskVar {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => Ok(TaskVar::String(String::from(s.to_str()?))),
            mlua::Value::Table(t) => {
                let mut result: HashMap<String, TaskVar> = HashMap::new();
                for pair in t.pairs() {
                    let (k, v): (String, TaskVar) = pair?;
                    result.insert(k, v);
                }
                Ok(TaskVar::Table(result))
            }
            _ => Err(mlua::Error::runtime(format!(
                "Expected a string or integer, but got a {}: {:?}",
                value.type_name(),
                value
            ))),
        }
    }
}

impl<'lua> mlua::IntoLua<'lua> for TaskVar {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        match self {
            TaskVar::Table(t) => {
                let lua_table = lua.create_table()?;
                for (k, v) in t {
                    lua_table.set(k, v)?;
                }
                Ok(mlua::Value::Table(lua_table))
            }
            TaskVar::List(l) => {
                let lua_table = lua.create_table()?;
                for (i, v) in l.into_iter().enumerate() {
                    lua_table.set(i + 1, v)?;
                }
                Ok(mlua::Value::Table(lua_table))
            }
            TaskVar::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        }
    }
}

// Implement a conversion function separate from serde since we want to convert non-string leaf types to strings, and
// it's easier to just implement
impl From<toml::Value> for TaskVar {
    fn from(value: toml::Value) -> Self {
        match value {
            toml::Value::Table(t) => {
                let mut tbl_var: HashMap<String, TaskVar> = HashMap::with_capacity(t.len());
                for (k, v) in t {
                    tbl_var.insert(k, v.into());
                }
                TaskVar::Table(tbl_var)
            }
            toml::Value::Array(arr) => {
                let mut list_var: Vec<TaskVar> = Vec::with_capacity(arr.len());
                for v in arr {
                    list_var.push(v.into());
                }
                TaskVar::List(list_var)
            }
            toml::Value::String(s) => TaskVar::String(s),
            toml::Value::Boolean(b) => TaskVar::String(format!("{}", b)),
            toml::Value::Datetime(dt) => TaskVar::String(format!("{}", dt)),
            toml::Value::Float(f) => TaskVar::String(format!("{}", f)),
            toml::Value::Integer(i) => TaskVar::String(format!("{}", i)),
        }
    }
}

pub fn lua_to_json<'lua>(
    lua: &'lua mlua::Lua,
    value: &mlua::Value<'lua>,
) -> mlua::Result<serde_json::Value> {
    use mlua::Value::*;
    let json_val = match value {
        Nil => serde_json::Value::Null,
        Boolean(b) => serde_json::Value::Bool(*b),
        Integer(i) => serde_json::Number::from_f64(*i as f64)
            .map(|n| serde_json::Value::Number(n))
            .unwrap_or(serde_json::Value::Null),
        Number(f) => serde_json::Number::from_f64(*f)
            .map(|n| serde_json::Value::Number(n))
            .unwrap_or(serde_json::Value::Null),
        String(s) => serde_json::Value::String(std::string::String::from(s.to_str()?)),
        Table(tbl) => {
            let is_sequence = validate_table_is_sequence(tbl, None, &mut Vec::new()).is_ok();
            if is_sequence {
                let mut arr: Vec<serde_json::Value> = Vec::new();
                for val_res in tbl.clone().sequence_values() {
                    let val: mlua::Value = val_res?;
                    let json_val = lua_to_json(lua, &val)?;
                    arr.push(json_val);
                }
                serde_json::Value::Array(arr)
            } else {
                let mut map: serde_json::Map<std::string::String, serde_json::Value> =
                    serde_json::Map::new();
                for pairs_res in tbl.clone().pairs() {
                    let (k, v): (mlua::Value, mlua::Value) = pairs_res?;
                    map.insert(k.to_string()?, lua_to_json(lua, &v)?);
                }
                serde_json::Value::Object(map)
            }
        }
        _ => serde_json::Value::String(value.to_string()?),
    };
    Ok(json_val)
}

pub fn json_to_lua<'lua>(
    lua: &'lua mlua::Lua,
    value: serde_json::Value,
) -> mlua::Result<mlua::Value<'lua>> {
    match value {
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                table.set(k, json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.into_iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(table))
        }
        serde_json::Value::Bool(b) => Ok(mlua::Value::Boolean(b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(|i| mlua::Value::Integer(i))
            .or_else(|| n.as_f64().map(|f| mlua::Value::Number(f)))
            .ok_or_else(|| mlua::Error::ToLuaConversionError {
                from: "json number",
                to: "lua integer or number",
                message: Some(format!("invalid value: {}", n)),
            }),
        serde_json::Value::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        serde_json::Value::Null => Ok(mlua::Value::Nil),
    }
}
