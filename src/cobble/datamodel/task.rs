use std::fmt;

use crate::cobble::datamodel::{
    Action,
    ActionCmd,
    Dependency,
    DependencyList,
    Artifact,
};

#[derive(Debug)]
pub struct TaskDef {
    pub build_env: String,
    pub actions: Vec<Action>,
    pub deps: Vec<Dependency>,
    pub artifacts: Vec<Artifact>
}

impl <'lua> mlua::FromLua<'lua> for TaskDef {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let task_def_table: mlua::Table = lua.unpack(value)?;
        let build_env: String = task_def_table.get("build_env")?;
        let actions: Vec<Action> = task_def_table.get("actions")?;
        let deps: Vec<Dependency> = task_def_table.get::<_, DependencyList>("deps")?.0;
        let artifacts: Vec<Artifact> = task_def_table.get("artifacts")?;

        Ok(TaskDef {
            build_env,
            actions,
            deps,
            artifacts
        })
    }
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub build_env: Option<String>,
    pub actions: Vec<ActionCmd>,
    pub deps: Vec<Dependency>,
    pub artifacts: Vec<Artifact>
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task(")?;
        write!(f, "name=\"{}\", ", self.name)?;
        
        if let Some(build_env) = &self.build_env {
            write!(f, "build_env=\"{}\", ", build_env)?;
        }

        f.write_str("actions=[")?;
        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", action)?;
        }
        f.write_str("], ")?;

        f.write_str("deps=[")?;
        for (i, dep) in self.deps.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", dep)?;
        }
        f.write_str("], ")?;

        f.write_str("artifacts=[")?;
        for (i, artifact) in self.artifacts.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", artifact)?;
        }
        f.write_str("])")
    }
}

impl <'lua> mlua::FromLua<'lua> for Task {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let task_table = lua.unpack::<mlua::Table>(value)?;
        let name: String = task_table.get("name")?;
        let build_env: Option<String> = task_table.get("build_env")?;
        let actions: Vec<ActionCmd> = task_table.get("actions")?;
        let deps: Vec<Dependency> = task_table.get::<_, DependencyList>("deps")?.0;
        let artifacts: Vec<Artifact> = task_table.get("artifacts")?;

        Ok(Task {
            name,
            build_env,
            actions,
            deps,
            artifacts
        })
    }
}
