// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::lua::s11n::{SerLuaValueBlock, SerLuaValueRef, SerLuaValueType};
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_string, validate_is_table,
    validate_table_has_only_string_or_sequence_keys, with_prop, ValidationError,
};

#[derive(Clone, Debug, Default)]
pub struct Dependencies {
    pub dirs: HashMap<Arc<str>, Arc<str>>,
    pub files: HashMap<Arc<str>, Arc<str>>,
    pub tasks: HashMap<Arc<str>, Arc<str>>,
    pub vars: HashMap<Arc<str>, Arc<str>>,
    pub calc: Vec<Arc<str>>,
}

impl fmt::Display for Dependencies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<'lua> mlua::FromLua<'lua> for Dependencies {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let value_block: SerLuaValueBlock = lua.unpack(value)?;
        let deps = Dependencies::try_from(&value_block).map_err(|e| mlua::Error::runtime(e))?;
        Ok(deps)
    }
}

pub fn validate_dep_list<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    match value {
        SerLuaValueRef::Table(dep_tbl) => {
            for (dep_type, dep_list) in dep_tbl.entries() {
                let dep_type_str = validate_is_string(&dep_type, &mut *prop_path)?;
                match dep_type_str {
                    "dirs" => with_prop(&mut *prop_path, SerLuaValueRef::String("dirs"), |path| {
                        let dirs_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(dirs_tbl, path)
                    }),
                    "files" => {
                        with_prop(&mut *prop_path, SerLuaValueRef::String("files"), |path| {
                            let files_tbl = validate_is_table(&dep_list, &mut *path)?;
                            validate_table_has_only_string_or_sequence_keys(files_tbl, path)
                        })
                    }
                    "tasks" => {
                        with_prop(&mut *prop_path, SerLuaValueRef::String("tasks"), |path| {
                            let tasks_tbl = validate_is_table(&dep_list, &mut *path)?;
                            validate_table_has_only_string_or_sequence_keys(tasks_tbl, path)
                        })
                    }
                    "vars" => with_prop(&mut *prop_path, SerLuaValueRef::String("vars"), |path| {
                        let vars_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(vars_tbl, path)
                    }),
                    "calc" => with_prop(&mut *prop_path, SerLuaValueRef::String("tasks"), |path| {
                        let calc_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(calc_tbl, path)
                    }),
                    _k => Err(ValidationError::InvalidKey {
                        path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                        expected: vec![
                            String::from("files"),
                            String::from("tasks"),
                            String::from("vars"),
                            String::from("calc"),
                        ],
                        key: dep_type.clone().into(),
                        table: value.clone().into(),
                    }),
                }?;
            }
            Ok(())
        }
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().cloned().map(|p| p.into()).collect(),
            expected: vec![SerLuaValueType::Table],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
    }
}

impl TryFrom<&SerLuaValueBlock> for Dependencies {
    type Error = String;

    fn try_from(value: &SerLuaValueBlock) -> Result<Self, Self::Error> {
        let mut deps: Dependencies = Default::default();

        let value_ref = SerLuaValueRef::from(&value.values, 0);

        let deps_table = value_ref
            .as_table()
            .ok_or_else(|| format!("Expected a table for dependencies"))?;

        for (dep_key, dep_val) in deps_table.entries() {
            if dep_key == SerLuaValueRef::String("dirs") {
                let dirs_table = dep_val
                    .as_table()
                    .ok_or_else(|| format!("dirs property must be a table"))?;

                for (k, v) in dirs_table.entries() {
                    let v_str = v
                        .as_string()
                        .map(|s| Arc::<str>::from(s))
                        .ok_or_else(|| format!("dir dependency must be a string"))?;
                    let k_str = match k {
                        SerLuaValueRef::String(s) => Arc::<str>::from(s),
                        SerLuaValueRef::Integer(_) => v_str.clone(),
                        _ => {
                            return Err(format!("dir dependency key must be a string or integer"));
                        }
                    };
                    deps.dirs.insert(k_str, v_str);
                }
            } else if dep_key == SerLuaValueRef::String("files") {
                let files_table = dep_val
                    .as_table()
                    .ok_or_else(|| format!("files property must be a table"))?;

                for (k, v) in files_table.entries() {
                    let v_str = v
                        .as_string()
                        .map(|s| Arc::<str>::from(s))
                        .ok_or_else(|| format!("file dependency must be a string"))?;
                    let k_str = match k {
                        SerLuaValueRef::String(s) => Arc::<str>::from(s),
                        SerLuaValueRef::Integer(_) => v_str.clone(),
                        _ => {
                            return Err(format!("file dependency key must be a string or integer"));
                        }
                    };
                    deps.files.insert(k_str, v_str);
                }
            } else if dep_key == SerLuaValueRef::String("tasks") {
                let tasks_table = dep_val
                    .as_table()
                    .ok_or_else(|| format!("tasks property must be a table"))?;

                for (k, v) in tasks_table.entries() {
                    let v_str = v
                        .as_string()
                        .map(|s| Arc::<str>::from(s))
                        .ok_or_else(|| format!("task dependency must be a string"))?;
                    let k_str = match k {
                        SerLuaValueRef::String(s) => Arc::<str>::from(s),
                        SerLuaValueRef::Integer(_) => v_str.clone(),
                        _ => {
                            return Err(format!("task dependency key must be a string or integer"));
                        }
                    };
                    deps.tasks.insert(k_str, v_str);
                }
            } else if dep_key == SerLuaValueRef::String("vars") {
                let vars_table = dep_val
                    .as_table()
                    .ok_or_else(|| format!("vars property must be a table"))?;

                for (k, v) in vars_table.entries() {
                    let v_str = v
                        .as_string()
                        .map(|s| Arc::<str>::from(s))
                        .ok_or_else(|| format!("var dependency must be a string"))?;
                    let k_str = match k {
                        SerLuaValueRef::String(s) => Arc::<str>::from(s),
                        SerLuaValueRef::Integer(_) => v_str.clone(),
                        _ => {
                            return Err(format!("var dependency key must be a string or integer"));
                        }
                    };
                    deps.vars.insert(k_str, v_str);
                }
            } else if dep_key == SerLuaValueRef::String("calc") {
                let calc_table = dep_val
                    .as_table()
                    .ok_or_else(|| format!("calc property must be a table"))?;

                for (_k, v) in calc_table.entries() {
                    let v_str = v
                        .as_string()
                        .map(|s| Arc::<str>::from(s))
                        .ok_or_else(|| format!("calc dependency must be a string"))?;
                    deps.calc.push(v_str);
                }
            }
        }

        Ok(deps)
    }
}
