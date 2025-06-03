// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::lua::s11n::{refify_ser_lua_value, SerLuaValueBlock, SerLuaValueRef};
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_string, validate_is_table,
    validate_table_has_only_string_or_sequence_keys,
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

pub fn validate_dep_list<'lua>(
    _lua: &'lua mlua::Lua,
    value: &mlua::Value,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    match value {
        mlua::Value::Table(dep_tbl) => {
            for pair in dep_tbl.clone().pairs() {
                let (dep_type, dep_list): (mlua::Value, mlua::Value) = pair?;
                let dep_type_str = validate_is_string(&dep_type, None, prop_path.as_mut())?;
                match dep_type_str.to_str()? {
                    "dirs" => validate_table_has_only_string_or_sequence_keys(
                        validate_is_table(
                            &dep_list,
                            Some(Cow::Borrowed("dirs")),
                            prop_path.as_mut(),
                        )?,
                        Some(Cow::Borrowed("dirs")),
                        prop_path.as_mut(),
                    ),
                    "files" => validate_table_has_only_string_or_sequence_keys(
                        validate_is_table(
                            &dep_list,
                            Some(Cow::Borrowed("files")),
                            prop_path.as_mut(),
                        )?,
                        Some(Cow::Borrowed("files")),
                        prop_path.as_mut(),
                    ),
                    "tasks" => validate_table_has_only_string_or_sequence_keys(
                        validate_is_table(
                            &dep_list,
                            Some(Cow::Borrowed("tasks")),
                            prop_path.as_mut(),
                        )?,
                        Some(Cow::Borrowed("tasks")),
                        prop_path.as_mut(),
                    ),
                    "vars" => validate_table_has_only_string_or_sequence_keys(
                        validate_is_table(
                            &dep_list,
                            Some(Cow::Borrowed("vars")),
                            prop_path.as_mut(),
                        )?,
                        Some(Cow::Borrowed("vars")),
                        prop_path.as_mut(),
                    ),
                    "calc" => validate_table_has_only_string_or_sequence_keys(
                        validate_is_table(
                            &dep_list,
                            Some(Cow::Borrowed("calc")),
                            prop_path.as_mut(),
                        )?,
                        Some(Cow::Borrowed("calc")),
                        prop_path.as_mut(),
                    ),
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

        let value_ref = refify_ser_lua_value(0, &value.values);

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
