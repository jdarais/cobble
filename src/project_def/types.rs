// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::lua::s11n::{SerLuaValue, SerLuaValueBlock, SerLuaValueRef};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StringOrInt {
    #[serde(rename="str")]
    String(Arc<str>),
    #[serde(rename="int")]
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

impl <'a> TryFrom<&SerLuaValueRef<'a>> for StringOrInt {
    type Error = String;
    
    fn try_from(value: &SerLuaValueRef<'a>) -> Result<Self, Self::Error> {
        match value {
            SerLuaValueRef::String(s) => Ok(StringOrInt::String(Arc::from(*s))),
            SerLuaValueRef::Integer(i) => Ok(StringOrInt::Int(*i)),
            _ => Err(format!("Expected a string or integer, but got a {}", value.value_type()))
        }
    }
}

impl TryFrom<SerLuaValue> for StringOrInt {
    type Error = String;

    fn try_from(value: SerLuaValue) -> Result<Self, Self::Error> {
        match value {
            SerLuaValue::String(s) => Ok(StringOrInt::String(Arc::from(s))),
            SerLuaValue::Integer(i) => Ok(StringOrInt::Int(i)),
            _ => Err(format!("Expected a string or integer, but got a {}", value.value_type()))
        }
    }
}

impl TryFrom<&SerLuaValueBlock> for StringOrInt {
    type Error = String;
    
    fn try_from(value: &SerLuaValueBlock) -> Result<Self, Self::Error> {
        StringOrInt::try_from(&SerLuaValueRef::from(&value.values, 0))
    }
}

impl mlua::FromLua for StringOrInt {
    fn from_lua(value: mlua::Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => Ok(StringOrInt::String(Arc::from(s.to_str()?.as_ref()))),
            mlua::Value::Integer(i) => Ok(StringOrInt::Int(i)),
            _ => Err(mlua::Error::runtime(format!(
                "Expected a string or integer, but got a {}: {:?}",
                value.type_name(),
                value
            ))),
        }
    }
}

impl mlua::IntoLua for StringOrInt {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        match self {
            StringOrInt::Int(i) => Ok(mlua::Value::Integer(i)),
            StringOrInt::String(s) => Ok(mlua::Value::String(lua.create_string(s.as_ref())?))
        }
    }
}

pub fn extend_string_or_int_table<I, T>(map: &mut BTreeMap<StringOrInt, T>, values: I) where I: Iterator<Item = (StringOrInt, T)> {
    for (k, v) in values {
        match k {
            StringOrInt::String(_) => { map.insert(k, v); }
            StringOrInt::Int(i) => {
                if !map.contains_key(&k) {
                    map.insert(k, v);
                } else {
                    let mut int_range = map.range(StringOrInt::Int(i)..StringOrInt::Int(i64::MAX));
                    let insert_index = match int_range.next_back() {
                        Some((StringOrInt::Int(range_i), _)) => range_i + 1,
                        Some((StringOrInt::String(_), _)) => { panic!("Iterating over the range of integers should not yield a string"); },
                        None => { panic!("Iterating over the range of integers should at least yield the conflicting index"); }
                    };
                    map.insert(StringOrInt::Int(insert_index), v);
                }
            }
        }
    }
}

pub fn json_to_lua(
    lua: &mlua::Lua,
    value: serde_json::Value,
) -> mlua::Result<mlua::Value> {
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
                from: String::from("json number"),
                to: "lua integer or number",
                message: Some(format!("invalid value: {}", n)),
            }),
        serde_json::Value::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        serde_json::Value::Null => Ok(mlua::Value::Nil),
    }
}
