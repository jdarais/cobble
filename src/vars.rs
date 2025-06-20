// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{error::Error, fmt};

#[derive(Debug)]
pub enum VarLookupError {
    InvalidName(String),
    PathComponentNotATable(String),
    PathNotFound(String),
}

impl Error for VarLookupError {}
impl fmt::Display for VarLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VarLookupError::*;
        match self {
            InvalidName(n) => write!(f, "Invalid variable name: {}", n),
            PathComponentNotATable(s) => write!(
                f,
                "Variable path component exists, but is not a table: {}",
                s
            ),
            PathNotFound(p) => write!(f, "Path in variable name does not exist: {}", p),
        }
    }
}

pub fn get_var<'a>(
    var_name: &str,
    vars: &'a serde_json::Map<String, serde_json::Value>,
) -> Result<&'a serde_json::Value, VarLookupError> {
    get_var_at_subpath_in_table(var_name, 0, vars)
}

fn get_var_at_subpath_in_table<'a>(
    var_name: &str,
    subpath_start: usize,
    table: &'a serde_json::Map<String, serde_json::Value>,
) -> Result<&'a serde_json::Value, VarLookupError> {
    let dot_idx_opt = var_name[subpath_start..].find(".");

    match dot_idx_opt {
        Some(dot_idx) => {
            // We should have a table at the next name component.
            // We want to search in that table with the remainder of the var name
            let key_name = &var_name[subpath_start..(subpath_start + dot_idx)];
            let subtable = table
                .get(key_name)
                .ok_or_else(|| VarLookupError::PathNotFound(String::from(var_name)))?;

            match subtable {
                serde_json::Value::Object(obj) => {
                    get_var_at_subpath_in_table(var_name, subpath_start + dot_idx + 1, &obj)
                }
                _ => Err(VarLookupError::PathComponentNotATable(
                    var_name[..(subpath_start + dot_idx)].to_owned(),
                )),
            }
        }
        None => {
            let key_name = &var_name[subpath_start..];
            table
                .get(key_name)
                .ok_or_else(|| VarLookupError::PathNotFound(String::from(var_name)))
        }
    }
}

pub fn set_var(
    var_name: &str,
    value: serde_json::Value,
    vars: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), VarLookupError> {
    set_var_at_subpath_in_table(var_name, 0, value, vars)
}

fn set_var_at_subpath_in_table(
    var_name: &str,
    subpath_start: usize,
    value: serde_json::Value,
    table: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), VarLookupError> {
    let dot_idx_opt = var_name[subpath_start..].find(".");

    match dot_idx_opt {
        Some(dot_idx) => {
            // We should have a table at the next name component.
            // We want to search in that table with the remainder of the var name
            let key_name = &var_name[subpath_start..(subpath_start + dot_idx)];
            if key_name.len() == 0 {
                return Err(VarLookupError::InvalidName(String::from(var_name)));
            }
            if !table.contains_key(key_name) {
                table.insert(
                    String::from(key_name),
                    serde_json::Value::Object(serde_json::Map::new()),
                );
            }

            match table.get_mut(key_name).unwrap() {
                serde_json::Value::Object(obj) => {
                    set_var_at_subpath_in_table(var_name, subpath_start + dot_idx + 1, value, obj)?;
                }
                _ => {
                    return Err(VarLookupError::PathComponentNotATable(
                        var_name[..(subpath_start + dot_idx)].to_owned(),
                    ));
                }
            }
        }
        None => {
            let key_name = &var_name[subpath_start..];
            if key_name.len() == 0 {
                return Err(VarLookupError::InvalidName(String::from(var_name)));
            }
            match table.get_mut(key_name) {
                Some(v) => *v = value,
                None => {
                    table.insert(key_name.to_owned(), value);
                }
            };
        }
    }

    Ok(())
}

pub fn unflatten_vars(
    vars: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Map<String, serde_json::Value>, VarLookupError> {
    let mut unflattened_vars: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

    for (k, v) in vars.iter() {
        set_var(k.as_str(), v.clone(), &mut unflattened_vars)?;
    }

    Ok(unflattened_vars)
}

pub fn extract_vars<'a, I, T>(
    var_paths: I,
    from_vars: &serde_json::Map<String, serde_json::Value>,
    to_vars: &mut serde_json::Map<String, serde_json::Value>
) -> Result<(), VarLookupError>
where
    I: Iterator<Item = T>,
    T: AsRef<str>,
{
    for var_path in var_paths {
        let var_value = get_var(var_path.as_ref(), from_vars)?;
        set_var(var_path.as_ref(), var_value.clone(), to_vars)?;
    }

    Ok(())
}
