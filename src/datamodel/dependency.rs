extern crate serde_json;

use std::{collections::HashMap, fmt, sync::Arc};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
enum StringOrInt {
    String(String),
    Int(i64)
}

#[derive(Serialize, Deserialize)]
struct DependencyListByType {
    files: Option<HashMap<StringOrInt, String>>,
    tasks: Option<HashMap<StringOrInt, String>>,
    calc: Option<HashMap<StringOrInt, String>>
}

pub struct DependencyList(pub Vec<Dependency>);

impl DependencyList {
    pub fn from_json(val: serde_json::Value) -> serde_json::Result<DependencyList> {
        let mut deps_by_type: DependencyListByType = serde_json::from_value(val)?;

        let mut deps: Vec<Dependency> = Vec::new();

        if let Some(mut files) = deps_by_type.files.take() {
            for (_, f) in files.drain() {
                deps.push(Dependency::File(f.into()));
            }
        }

        if let Some(mut tasks) = deps_by_type.tasks.take() {
            for (_, t) in tasks.drain() {
                deps.push(Dependency::Task(t.into()));
            }
        }

        if let Some(mut calc_deps) = deps_by_type.calc.take() {
            for (_, c) in calc_deps.drain() {
                deps.push(Dependency::Calc(c.into()));
            }
        }

        Ok(DependencyList(deps))
    }
}

impl <'lua> mlua::FromLua<'lua> for DependencyList {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let mut file_deps: Option<Vec<String>> = None;
        let mut task_deps: Option<Vec<String>> = None;
        let mut calc_deps: Option<Vec<String>> = None;

        let deps_table: mlua::Table = lua.unpack(value)?;
        for pair in deps_table.pairs() {
            let (k, v): (String, mlua::Value) = pair?;
            match k.as_str() {
                "files" => { file_deps = lua.unpack(v)?; },
                "tasks" => { task_deps = lua.unpack(v)?; },
                "calc" => { calc_deps = lua.unpack(v)?; },
                _ => { return Err(mlua::Error::runtime(format!("Unknown dependency type: {}", k))); }
            }
        }

        let deps: Vec<Dependency> = file_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|f| Dependency::File(f.into()))
            .chain(task_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|t| Dependency::Task(t.into())))
            .chain(calc_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|c| Dependency::Calc(c.into())))
            .collect();

        Ok(DependencyList(deps))
    }
}

#[derive(Clone, Debug)]
pub enum Dependency {
    File(Arc<str>),
    Task(Arc<str>),
    Calc(Arc<str>)
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Dependency::*;
        match self {
            File(val) => write!(f, "File({})", val.as_ref()),
            Task(val) => write!(f, "Task({})", val.as_ref()),
            Calc(val) => write!(f, "Calc({})", val.as_ref())
        }
    }
}
