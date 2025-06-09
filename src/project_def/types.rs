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
