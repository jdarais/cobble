// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use mlua::FromLua;
use std::borrow::Cow;

use crate::lua::s11n::SerLuaValueBlock;

pub fn prop_path_string(prop_path: &Vec<Cow<'static, str>>) -> String {
    prop_path.join(".")
}

pub fn prop_key_string(
    lua: &mlua::Lua,
    key: &mlua::Value,
) -> mlua::Result<Cow<'static, str>> {
    match key {
        mlua::Value::String(s) => Ok(Cow::Owned(String::from(s.to_str()?.as_ref()))),
        _ => {
            let ser_key = SerLuaValueBlock::from_lua(key.clone(), lua)?;
            Ok(Cow::Owned(format!("[{ser_key}]")))
        }
    }
}

pub fn with_prop<T, F: FnOnce(&mut Vec<Cow<'static, str>>) -> T>(
    prop_path: &mut Vec<Cow<'static, str>>,
    prop: Cow<'static, str>,
    func: F,
) -> T {
    prop_path.push(prop);
    let res = func(prop_path);
    prop_path.pop();
    res
}

pub fn validate_table_has_only_string_or_sequence_keys(
    table: &mlua::Table,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, _v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::String(_) => Ok(()),
            mlua::Value::Integer(i) => {
                if i >= 1 && i <= sequence_len {
                    Ok(())
                } else {
                    Err(mlua::Error::runtime(format!(
                        "In {}: Disjoint number indices not allowed: {}",
                        prop_path_string(prop_path.as_mut()),
                        i
                    )))
                }
            }
            _ => Err(mlua::Error::runtime(format!(
                "In {}: Expected string or integer index, but got a {}: {:?}",
                prop_path_string(prop_path.as_mut()),
                k.type_name(),
                k
            ))),
        }?;
    }
    Ok(())
}

pub fn validate_table_is_sequence(
    table: &mlua::Table,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, _v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::Integer(i) => {
                if i >= 1 && i <= sequence_len {
                    Ok(())
                } else {
                    Err(mlua::Error::runtime(format!(
                        "In {}: Sequence expected, but disjoint integer index found: {}",
                        prop_path_string(prop_path.as_mut()),
                        i
                    )))
                }
            }
            _ => Err(mlua::Error::runtime(format!(
                "In {}: Sequence expected, but non-integer index found: {:?}",
                prop_path_string(prop_path.as_mut()),
                k
            ))),
        }?;
    }

    Ok(())
}

pub fn validate_is_string<'a>(
    value: &'a mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<&'a mlua::String> {
    match value {
        mlua::Value::String(s) => Ok(s),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a string, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn validate_is_bool(
    value: &mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<bool> {
    match value {
        mlua::Value::Boolean(b) => Ok(*b),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a boolean, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn validate_is_table<'a>(
    value: &'a mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<&'a mlua::Table> {
    match value {
        mlua::Value::Table(t) => Ok(t),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a table, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn key_validation_error<T>(
    key: &str,
    valid_keys: Vec<&str>,
    prop_path: &Vec<Cow<'static, str>>,
) -> mlua::Result<T> {
    Err(mlua::Error::runtime(format!(
        "In {}: Unknonwn property name: '{}'. Expected one of [{}]",
        prop_path_string(prop_path),
        key,
        valid_keys.join(", ")
    )))
}

pub fn validate_required_key(
    table: &mlua::Table,
    key: &str,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    if table.contains_key(key)? {
        Ok(())
    } else {
        Err(mlua::Error::runtime(format!(
            "In {}: '{}' property is required",
            prop_path_string(prop_path.as_mut()),
            key
        )))
    }
}
