use std::fmt;

pub struct DependencyList(pub Vec<Dependency>);

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

        let deps: Vec<Dependency> = file_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|f| Dependency::File(f))
            .chain(task_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|t| Dependency::Task(t)))
            .chain(calc_deps.unwrap_or_else(|| Vec::new()).into_iter().map(|c| Dependency::Calc(c)))
            .collect();

        Ok(DependencyList(deps))
    }
}

#[derive(Clone, Debug)]
pub enum Dependency {
    File(String),
    Task(String),
    Calc(String)
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Dependency::*;
        match self {
            File(val) => write!(f, "File({})", val.as_str()),
            Task(val) => write!(f, "Task({})", val.as_str()),
            Calc(val) => write!(f, "Calc({})", val.as_str())
        }
    }
}
