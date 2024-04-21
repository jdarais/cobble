use std::fmt;

pub struct DependencyList(pub Vec<Dependency>);

impl <'lua> mlua::FromLua<'lua> for DependencyList {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let deps_table: mlua::Table = lua.unpack(value)?;
        let file_deps: Option<Vec<String>> = deps_table.get("files")?;
        let task_deps: Option<Vec<String>> = deps_table.get("tasks")?;
        let deps: Vec<Dependency> = file_deps.unwrap_or(vec![]).into_iter().map(|f| Dependency::File(f))
            .chain(task_deps.unwrap_or(vec![]).into_iter().map(|t| Dependency::Task(t)))
            .collect();

        Ok(DependencyList(deps))
    }
}

#[derive(Clone, Debug)]
pub enum Dependency {
    File(String),
    Task(String)
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Dependency::*;
        match self {
            File(val) => write!(f, "File({})", val.as_str()),
            Task(val) => write!(f, "Task({})", val.as_str())
        }
    }
}
