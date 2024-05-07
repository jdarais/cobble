use std::{collections::HashMap, fmt};

use crate::datamodel::types::TaskVar;

#[derive(Debug)]
pub enum VarLookupError {
    InvalidName(String),
    PathComponentNotATable(String),
    PathNotFound(String)
}

impl fmt::Display for VarLookupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use VarLookupError::*;
        match self {
            InvalidName(n) => write!(f, "Invalid variable name: {}", n),
            PathComponentNotATable(s) => write!(f, "Variable path component exists, but is not a table: {}", s),
            PathNotFound(p) => write!(f, "Path in variable name does not exist: {}", p),
        }
    }
}

pub fn get_var<'a>(var_name: &str, vars: &'a HashMap<String, TaskVar>) -> Result<&'a TaskVar, VarLookupError> {
    get_var_at_subpath_in_table(var_name, 0, vars)
}

fn get_var_at_subpath_in_table<'a>(var_name: &str, subpath_start: usize, table: &'a HashMap<String, TaskVar>) -> Result<&'a TaskVar, VarLookupError> {
    let dot_idx_opt = var_name[subpath_start..].find(".");

    match dot_idx_opt {
        Some(dot_idx) => {
            // We should have a table at the next name component.
            // We want to search in that table with the remainder of the var name
            let key_name = &var_name[subpath_start..(subpath_start+dot_idx)];
            let subtable = table.get(key_name)
                .ok_or_else(|| VarLookupError::PathNotFound(String::from(var_name)))?;

            match subtable {
                TaskVar::Table(t) => get_var_at_subpath_in_table(var_name, subpath_start + dot_idx + 1, &t),
                _ => Err(VarLookupError::PathComponentNotATable(var_name[..(subpath_start+dot_idx)].to_owned()))
            }
        },
        None => {
            let key_name = &var_name[subpath_start..];
            table.get(key_name)
                .ok_or_else(|| VarLookupError::PathNotFound(String::from(var_name)))
        }
    }
}

pub fn set_var(var_name: &str, value: TaskVar, vars: &mut HashMap<String, TaskVar>) -> Result<(), VarLookupError> {
    set_var_at_subpath_in_table(var_name, 0, value, vars)
}

fn set_var_at_subpath_in_table(var_name: &str, subpath_start: usize, value: TaskVar, table: &mut HashMap<String, TaskVar>) -> Result<(), VarLookupError> {
    let dot_idx_opt = var_name[subpath_start..].find(".");

    match dot_idx_opt {
        Some(dot_idx) => {
            // We should have a table at the next name component.
            // We want to search in that table with the remainder of the var name
            let key_name = &var_name[subpath_start..(subpath_start+dot_idx)];
            if key_name.len() == 0 {
                return Err(VarLookupError::InvalidName(String::from(var_name)));
            }
            if !table.contains_key(key_name) {
                table.insert(String::from(key_name), TaskVar::Table(HashMap::new()));
            }

            match table.get_mut(key_name).unwrap() {
                TaskVar::Table(t) => {
                    set_var_at_subpath_in_table(var_name, subpath_start + dot_idx + 1, value, t)?;
                },
                _ => { return Err(VarLookupError::PathComponentNotATable(var_name[..(subpath_start+dot_idx)].to_owned())); }
            }
        },
        None => {
            let key_name = &var_name[subpath_start..];
            if key_name.len() == 0 {
                return Err(VarLookupError::InvalidName(String::from(var_name)));
            }
            match table.get_mut(key_name) {
                Some(v) => { *v = value },
                None => { table.insert(key_name.to_owned(), value); }
            };
        }
    }

    Ok(())
}
