// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::project_def::types::StringOrInt;
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_string, validate_is_table,
    validate_table_has_only_string_or_sequence_keys,
};

use super::types::MapOrArray;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DependencyListByType {
    pub dirs: Option<MapOrArray<String>>,
    pub files: Option<MapOrArray<String>>,
    pub tasks: Option<MapOrArray<String>>,
    pub vars: Option<MapOrArray<String>>,
    pub calc: Option<MapOrArray<String>>,
}

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
        let deps_by_type: DependencyListByType = lua.unpack(value)?;
        Ok(deps_by_type.into())
    }
}

fn alias_map_from_map_or_array(value: MapOrArray<String>) -> HashMap<Arc<str>, Arc<str>> {
    match value {
        MapOrArray::Map(m) => m.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        MapOrArray::Array(arr) => arr
            .into_iter()
            .map(|v| {
                let val: Arc<str> = v.into();
                (val.clone(), val)
            })
            .collect(),
    }
}

impl From<DependencyListByType> for Dependencies {
    fn from(value: DependencyListByType) -> Self {
        let DependencyListByType {
            dirs,
            files,
            tasks,
            vars,
            calc,
        } = value;

        let calc_deps_list: Vec<Arc<str>> = match calc {
            Some(MapOrArray::Map(m)) => m.into_iter().map(|(_k, v)| v.into()).collect(),
            Some(MapOrArray::Array(arr)) => arr.into_iter().map(|v| v.into()).collect(),
            _ => Vec::new()
        };

        Dependencies {
            dirs: dirs.map(alias_map_from_map_or_array).unwrap_or_default(),
            files: files.map(alias_map_from_map_or_array).unwrap_or_default(),
            tasks: tasks.map(alias_map_from_map_or_array).unwrap_or_default(),
            vars: vars.map(alias_map_from_map_or_array).unwrap_or_default(),
            calc: calc_deps_list,
        }
    }
}

fn write_string_or_int_map(
    f: &mut fmt::Formatter<'_>,
    val: &HashMap<StringOrInt, String>,
) -> fmt::Result {
    for (i, (f_alias, f_path)) in val.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{}: {}", f_alias, f_path)?;
    }
    Ok(())
}

impl fmt::Display for DependencyListByType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;

        if let Some(files) = &self.files {
            f.write_str("files={")?;
            write_string_or_int_map(f, &files.clone().into())?;
            f.write_str("},")?;
        }

        if let Some(tasks) = &self.tasks {
            f.write_str("tasks={")?;
            write_string_or_int_map(f, &tasks.clone().into())?;
            f.write_str("},")?;
        }

        if let Some(vars) = &self.vars {
            f.write_str("vars={")?;
            write_string_or_int_map(f, &vars.clone().into())?;
            f.write_str("},")?;
        }

        if let Some(calc) = &self.calc {
            f.write_str("calc={")?;
            write_string_or_int_map(f, &calc.clone().into())?;
            f.write_str("}")?;
        }

        f.write_str("}")
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

impl<'lua> mlua::FromLua<'lua> for DependencyListByType {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let mut deps = DependencyListByType {
            dirs: None,
            files: None,
            tasks: None,
            vars: None,
            calc: None,
        };

        let deps_table: mlua::Table = lua.unpack(value)?;
        for pair in deps_table.pairs() {
            let (k, v): (String, mlua::Value) = pair?;
            match k.as_str() {
                "dirs" => {
                    deps.dirs = lua.unpack(v)?;
                }
                "files" => {
                    deps.files = lua.unpack(v)?;
                }
                "tasks" => {
                    deps.tasks = lua.unpack(v)?;
                }
                "vars" => {
                    deps.vars = lua.unpack(v)?;
                }
                "calc" => {
                    deps.calc = lua.unpack(v)?;
                }
                _ => {
                    return Err(mlua::Error::runtime(format!(
                        "Unknown dependency type: {}",
                        k
                    )));
                }
            }
        }

        Ok(deps)
    }
}
