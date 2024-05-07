extern crate serde_json;

use std::{collections::HashMap, fmt, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::datamodel::types::StringOrInt;
use crate::datamodel::validate::{key_validation_error, validate_is_string, validate_is_table, validate_table_has_only_string_or_sequence_keys};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DependencyListByType {
    pub files: Option<HashMap<StringOrInt, String>>,
    pub tasks: Option<HashMap<StringOrInt, String>>,
    pub vars: Option<HashMap<StringOrInt, String>>,
    pub calc: Option<HashMap<StringOrInt, String>>
}

#[derive(Clone, Debug, Default)]
pub struct Dependencies {
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

impl <'lua> mlua::FromLua<'lua> for Dependencies {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let deps_by_type: DependencyListByType = lua.unpack(value)?;
        Ok(deps_by_type.into())
    }
}

fn alias_map_from_string_or_int_map(value: HashMap<StringOrInt, String>) -> HashMap<Arc<str>, Arc<str>> {
    let mut result = HashMap::with_capacity(value.len());
    for (k, v) in value {
        match k {
            StringOrInt::Int(_i) => {
                let f_dep = Arc::<str>::from(v);
                result.insert(f_dep.clone(), f_dep); 
            },
            StringOrInt::String(s) => {
                result.insert(s.into(), v.into());
            }
        }
    }
    result
}

impl From<DependencyListByType> for Dependencies {
    fn from(value: DependencyListByType) -> Self {
        let DependencyListByType { files, tasks, vars, calc } = value;

        let mut calc_deps_list: Vec<Arc<str>> = Vec::with_capacity(calc.as_ref().map(|c| c.len()).unwrap_or(0));
        if let Some(c_deps) = calc {
            for (_k, v) in c_deps {
                calc_deps_list.push(v.into());
            }
        }

        Dependencies {
            files: files.map(alias_map_from_string_or_int_map).unwrap_or_default(),
            tasks: tasks.map(alias_map_from_string_or_int_map).unwrap_or_default(),
            vars: vars.map(alias_map_from_string_or_int_map).unwrap_or_default(),
            calc: calc_deps_list
        }
    }
}

fn write_string_or_int_map(f: &mut fmt::Formatter<'_>, val: &HashMap<StringOrInt, String>) -> fmt::Result {
    for (i, (f_alias, f_path)) in val.iter().enumerate() {
        if i > 0 { f.write_str(", ")?; }
        write!(f, "{}: {}", f_alias, f_path)?;
    }
    Ok(())
}

impl fmt::Display for DependencyListByType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;

        if let Some(files) = &self.files {
            f.write_str("files={")?;
            write_string_or_int_map(f, &files)?;
            f.write_str("},")?;
        }

        if let Some(tasks) = &self.tasks {
            f.write_str("tasks={")?;
            write_string_or_int_map(f, &tasks)?;
            f.write_str("},")?;
        }

        if let Some(vars) = &self.vars {
            f.write_str("vars={")?;
            write_string_or_int_map(f, &vars)?;
            f.write_str("},")?;
        }

        if let Some(calc) = &self.calc {
            f.write_str("calc={")?;
            write_string_or_int_map(f, &calc)?;
            f.write_str("}")?;
        }

        f.write_str("}")
    }
}

pub fn validate_dep_list<'lua>(_lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    match value {
        mlua::Value::Table(dep_tbl) => {
            for pair in dep_tbl.clone().pairs() {
                let (dep_type, dep_list): (mlua::Value, mlua::Value) = pair?;
                let dep_type_str = validate_is_string(&dep_type)?;
                match dep_type_str.to_str()? {
                    "files" => validate_table_has_only_string_or_sequence_keys(validate_is_table(&dep_list)?),
                    "tasks" => validate_table_has_only_string_or_sequence_keys(validate_is_table(&dep_list)?),
                    "vars" => validate_table_has_only_string_or_sequence_keys(validate_is_table(&dep_list)?),
                    "calc" => validate_table_has_only_string_or_sequence_keys(validate_is_table(&dep_list)?),
                    key => key_validation_error(key, vec!["files", "tasks", "vars", "calc"])
                }?;
            }
            Ok(())
        },
        _ => Err(mlua::Error::runtime(format!("Expected a table, but got a {}: {:?}", value.type_name(), value)))
    }
}

impl <'lua> mlua::FromLua<'lua> for DependencyListByType {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let mut deps = DependencyListByType {
            files: None,
            tasks: None,
            vars: None,
            calc: None
        };

        let deps_table: mlua::Table = lua.unpack(value)?;
        for pair in deps_table.pairs() {
            let (k, v): (String, mlua::Value) = pair?;
            match k.as_str() {
                "files" => { deps.files = lua.unpack(v)?; },
                "tasks" => { deps.tasks = lua.unpack(v)?; },
                "vars" => { deps.vars = lua.unpack(v)?; },
                "calc" => { deps.calc = lua.unpack(v)?; },
                _ => { return Err(mlua::Error::runtime(format!("Unknown dependency type: {}", k))); }
            }
        }

        Ok(deps)
    }
}
