extern crate serde;
extern crate mlua;

use std::fmt;

use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum StringOrInt {
    String(String),
    Int(i64)
}

impl fmt::Display for StringOrInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringOrInt::String(s) => write!(f, "\"{}\"", s),
            StringOrInt::Int(i) => write!(f, "{}", i)
        }
    }
}

impl <'lua> mlua::FromLua<'lua> for StringOrInt {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => Ok(StringOrInt::String(String::from(s.to_str()?))),
            mlua::Value::Integer(i) => Ok(StringOrInt::Int(i)),
            _ => Err(mlua::Error::runtime(format!("Expected a string or integer, but got a {}: {:?}", value.type_name(), value)))
        }
    }
}
