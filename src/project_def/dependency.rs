// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::{fmt, sync::Arc};

use crate::lua::s11n::{refify_ser_lua_value, SerLuaValueBlock, SerLuaValueRef};
use crate::project_def::types::StringOrInt;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_is_table,
    validate_table_has_only_string_or_sequence_keys, with_prop,
};

#[derive(Clone, Debug, Default)]
pub struct Dependencies {
    pub dirs: BTreeMap<StringOrInt, Arc<str>>,
    pub files: BTreeMap<StringOrInt, Arc<str>>,
    pub tasks: BTreeMap<StringOrInt, Arc<str>>,
    pub vars: Vec<Arc<str>>,
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

pub fn validate_dep_list<'lua>(
    _lua: &'lua mlua::Lua,
    value: &mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    match value {
        mlua::Value::Table(dep_tbl) => {
            for pair in dep_tbl.clone().pairs() {
                let (dep_type, dep_list): (mlua::Value, mlua::Value) = pair?;
                let dep_type_str = validate_is_string(&dep_type, &mut *prop_path)?;
                match dep_type_str.to_str()? {
                    "dirs" => with_prop(&mut *prop_path, Cow::Borrowed("dirs"), |path| {
                        let dirs_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(&dirs_tbl, path)
                    }),
                    "files" => with_prop(&mut *prop_path, Cow::Borrowed("files"), |path| {
                        let files_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(&files_tbl, path)
                    }),
                    "tasks" => with_prop(&mut *prop_path, Cow::Borrowed("tasks"), |path| {
                        let tasks_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(&tasks_tbl, path)
                    }),
                    "vars" => with_prop(&mut *prop_path, Cow::Borrowed("vars"), |path| {
                        let vars_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(&vars_tbl, path)
                    }),
                    "calc" => with_prop(&mut *prop_path, Cow::Borrowed("calc"), |path| {
                        let calc_tbl = validate_is_table(&dep_list, &mut *path)?;
                        validate_table_has_only_string_or_sequence_keys(&calc_tbl, path)
                    }),
                    key => key_validation_error(
                        key,
                        vec!["files", "tasks", "vars", "calc"],
                        prop_path.as_mut(),
                    ),
                }?;
            }
            Ok(())
        }
        _ => Err(mlua::Error::runtime(format!(
            "Expected a table, but got a {}: {:?}",
            value.type_name(),
            value
        ))),
    }
}

impl TryFrom<&SerLuaValueBlock> for Dependencies {
    type Error = String;

    fn try_from(value: &SerLuaValueBlock) -> Result<Self, Self::Error> {
        let mut deps: Dependencies = Default::default();

        let value_ref = refify_ser_lua_value(&value.values, 0);

        let deps_table = value_ref
            .as_table()
            .ok_or_else(|| format!("Expected a table for dependencies"))?;

        for (dep_key, dep_val) in deps_table.entries() {
            match dep_key {
                SerLuaValueRef::String("dirs") => {
                    let dirs_table = dep_val
                        .as_table()
                        .ok_or_else(|| format!("dirs property must be a table"))?;

                    for (k, v) in dirs_table.entries() {
                        let v_str = v
                            .as_string()
                            .map(|s| Arc::<str>::from(s))
                            .ok_or_else(|| format!("dir dependency must be a string"))?;
                        let k_val = StringOrInt::try_from(&k)?;
                        deps.dirs.insert(k_val, v_str);
                    }
                }
                SerLuaValueRef::String("files") => {
                    let files_table = dep_val
                        .as_table()
                        .ok_or_else(|| format!("files property must be a table"))?;

                    for (k, v) in files_table.entries() {
                        let v_str = v
                            .as_string()
                            .map(|s| Arc::<str>::from(s))
                            .ok_or_else(|| format!("file dependency must be a string"))?;
                        let k_val = StringOrInt::try_from(&k)?;
                        deps.files.insert(k_val, v_str);
                    }
                }
                SerLuaValueRef::String("tasks") => {
                    let tasks_table = dep_val
                        .as_table()
                        .ok_or_else(|| format!("tasks property must be a table"))?;

                    for (k, v) in tasks_table.entries() {
                        let v_str = v
                            .as_string()
                            .map(|s| Arc::<str>::from(s))
                            .ok_or_else(|| format!("task dependency must be a string"))?;
                        let k_val = StringOrInt::try_from(&k)?;
                        deps.tasks.insert(k_val, v_str);
                    }
                }
                SerLuaValueRef::String("vars") => {
                    let vars_table = dep_val
                        .as_table()
                        .ok_or_else(|| format!("vars property must be a table"))?;

                    for (_k, v) in vars_table.entries() {
                        let v_str = v
                            .as_string()
                            .map(|s| Arc::<str>::from(s))
                            .ok_or_else(|| format!("var dependency must be a string"))?;
                        deps.vars.push(v_str);
                    }
                }
                SerLuaValueRef::String("calc") => {
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
                _ => { return Err(format!("Unknown dependency type: {dep_key}")); }
            }
        }

        Ok(deps)
    }
}
