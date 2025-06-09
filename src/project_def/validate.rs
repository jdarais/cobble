// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{
    borrow::Cow,
    fmt::{self, Write},
};

use crate::{
    lua::s11n::{SerLuaTableRef, SerLuaValue, SerLuaValueBlock, SerLuaValueRef, SerLuaValueType},
    util::onscopeexit::OnScopeExitMut,
};

pub enum ValidationError {
    InvalidType {
        path: Vec<SerLuaValueBlock>,
        expected: Vec<SerLuaValueType>,
        actual: SerLuaValueType,
        value: SerLuaValueBlock,
    },
    InvalidKey {
        path: Vec<SerLuaValueBlock>,
        expected: Vec<String>,
        key: SerLuaValueBlock,
        table: SerLuaValueBlock,
    },
    MissingKey {
        path: Vec<SerLuaValueBlock>,
        key: String,
        table: SerLuaValueBlock,
    },
    InvalidValue {
        path: Vec<SerLuaValueBlock>,
        value: SerLuaValueBlock,
        message: String,
    },
}

fn to_key_str(value: &SerLuaValueBlock) -> String {
    if value.values.len() == 1 {
        if let SerLuaValue::String(s) = &value.values[0] {
            return s.clone();
        }
    }

    let value_str = format!("[{value}]");
    value_str
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ValidationError::*;
        match self {
            InvalidType {
                path,
                expected,
                actual,
                value,
            } => {
                let path_key_list: Vec<String> = path.iter().map(|p| to_key_str(p)).collect();
                let path_str = path_key_list.join(".");
                let mut expected_str = String::from("[");
                for (i, exp) in expected.iter().enumerate() {
                    if i > 0 {
                        expected_str.push_str(", ");
                    }
                    write!(&mut expected_str, "{}", exp)?;
                }
                expected_str.push_str("]");
                write!(f, "At {path_str}: Invalid type {actual}. Expected one of {expected_str}. (value={value})")
            }
            InvalidKey {
                path,
                expected,
                key,
                table,
            } => {
                let path_key_list: Vec<String> = path.iter().map(|p| to_key_str(p)).collect();
                let path_str = path_key_list.join(".");
                let expected_list_str = expected.join(", ");
                let expected_str = format!("[{expected_list_str}]");
                write!(f, "At {path_str}: Invalid key {key}. Expected one of {expected_str}. (table={table})")
            }
            MissingKey { path, key, table } => {
                let path_key_list: Vec<String> = path.iter().map(|p| to_key_str(p)).collect();
                let path_str = path_key_list.join(".");
                write!(
                    f,
                    "At {path_str}: Key {key} is required, but was missing. (table={table})"
                )
            }
            InvalidValue {
                path,
                value,
                message,
            } => {
                let path_key_list: Vec<String> = path.iter().map(|p| to_key_str(p)).collect();
                let path_str = path_key_list.join(".");
                write!(f, "At {path_str}: {message} (value={value}")
            }
        }
    }
}

pub fn prop_path_string(prop_path: &Vec<Cow<'static, str>>) -> String {
    prop_path.join(".")
}

pub fn with_prop<'a, T, F: FnOnce(&mut Vec<SerLuaValueRef<'a>>) -> T>(
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
    prop: SerLuaValueRef<'a>,
    func: F,
) -> T {
    prop_path.push(prop);
    let res = func(prop_path);
    prop_path.pop();
    res
}

pub fn push_prop_name_if_exists<'a>(
    prop_name: Option<Cow<'static, str>>,
    prop_path: &'a mut Vec<Cow<'static, str>>,
) -> OnScopeExitMut<'a, Vec<Cow<'static, str>>> {
    match prop_name {
        Some(name) => {
            prop_path.push(name);
            OnScopeExitMut::new(
                prop_path,
                Box::new(|path| {
                    path.pop();
                }),
            )
        }
        None => OnScopeExitMut::new(prop_path, Box::new(|_| {})),
    }
}

pub fn validate_is_type<'a>(
    value: &SerLuaValueRef<'a>,
    valid_types: &Vec<SerLuaValueType>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let is_valid = valid_types
        .iter()
        .find(|t| **t == value.value_type())
        .is_none();

    if is_valid {
        return Err(ValidationError::InvalidType {
            path: prop_path
                .iter()
                .map(|v| SerLuaValueBlock::from(v.clone()))
                .collect(),
            expected: valid_types.clone(),
            actual: value.value_type(),
            value: value.clone().into(),
        });
    }

    Ok(())
}

pub fn validate_table_value_type<'a>(
    table: &SerLuaTableRef<'a>,
    valid_types: &Vec<SerLuaValueType>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    for (k, v) in table.entries() {
        with_prop(prop_path, k, |path|
            validate_is_type(
                &v,
                valid_types,
                path,
            )
        )?;
    }

    Ok(())
}

pub fn validate_table_has_only_string_or_sequence_keys<'a>(
    table: &SerLuaTableRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let entries: Vec<(SerLuaValueRef<'a>, SerLuaValueRef<'a>)> = table.entries().collect();
    let entries_len = entries.len();
    for (k, _v) in entries {
        match k {
            SerLuaValueRef::String(_) => Ok(()),
            SerLuaValueRef::Integer(i) => {
                if i >= 1 && i as usize <= entries_len {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidValue {
                        path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
                        value: SerLuaValueRef::Table(table.clone()).into(),
                        message: String::from("Sequence (integer) keys must be contiguous"),
                    })
                }
            }
            _ => Err(ValidationError::InvalidValue {
                path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
                value: SerLuaValueRef::Table(table.clone()).into(),
                message: format!(
                    "Invalid key type {}. Only string or sequence (integer) keys allowed. (key={})",
                    k.value_type(),
                    k
                ),
            }),
        }?;
    }
    Ok(())
}

pub fn validate_table_is_sequence<'a>(
    table: &SerLuaTableRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let entries: Vec<(SerLuaValueRef<'a>, SerLuaValueRef<'a>)> = table.entries().collect();
    for (k, _v) in table.entries() {
        match k {
            SerLuaValueRef::Integer(i) => {
                if i >= 1 && i as usize <= entries.len() {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidValue {
                        path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
                        value: SerLuaValueRef::Table(table.clone()).into(),
                        message: String::from("Sequence expected, but indices are not contiguous"),
                    })
                }
            }
            _ => Err(ValidationError::InvalidValue {
                path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
                value: SerLuaValueRef::Table(table.clone()).into(),
                message: format!("Sequence expected, but non-integer key found: {k}"),
            }),
        }?;
    }

    Ok(())
}

pub fn validate_is_string<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<&'a str, ValidationError> {
    match value {
        SerLuaValueRef::String(s) => Ok(s),
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
            expected: vec![SerLuaValueType::String],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
    }
}

pub fn validate_is_bool<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<bool, ValidationError> {
    match value {
        SerLuaValueRef::Boolean(b) => Ok(*b),
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
            expected: vec![SerLuaValueType::Boolean],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
    }
}

pub fn validate_is_table<'v, 'a>(
    value: &'v SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<&'v SerLuaTableRef<'a>, ValidationError> {
    match value {
        SerLuaValueRef::Table(t) => Ok(t),
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
            expected: vec![SerLuaValueType::Table],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
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

pub fn validate_required_key<'a>(
    table: &SerLuaTableRef<'a>,
    key: &str,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    table
        .entries()
        .find(|(k, _v)| *k == SerLuaValueRef::String(key))
        .map(|_| ())
        .ok_or_else(|| ValidationError::MissingKey {
            path: prop_path.iter().map(|v| SerLuaValueBlock::from(v.clone())).collect(),
            key: key.to_owned(),
            table: SerLuaValueRef::Table(table.clone()).into(),
        })
}
