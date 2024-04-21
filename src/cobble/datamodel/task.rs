use std::{collections::HashMap, fmt};

use crate::cobble::datamodel::{
    Action,
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

#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub build_env: Option<(String, String)>,
    pub actions: Vec<Action>,
    pub deps: Vec<Dependency>,
    pub artifacts: Vec<Artifact>
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task(")?;
        write!(f, "name=\"{}\", ", self.name)?;
        
        if let Some((env_alias, env_name)) = &self.build_env {
            write!(f, "build_env={{\"{}\": \"{}\"}}, ", env_alias, env_name)?;
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
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(task_table) => {
                let name: String = task_table.get("name")?;

                let build_env_val: mlua::Value = task_table.get("build_env")?;
                let build_env = match build_env_val {
                    mlua::Value::String(s) => Some((String::from(s.to_str()?), String::from(s.to_str()?))),
                    mlua::Value::Table(t) => {
                        let mut envs: HashMap<String, String> = HashMap::new();
                        for pair in t.pairs() {
                            let (k, v): (String, String) = pair?;
                            envs.insert(k, v);
                        }

                        if envs.len() > 1 {
                            return Err(mlua::Error::runtime("Only one build env can be assigned at the task level"));
                        }

                        envs.into_iter().next()
                    },
                    mlua::Value::Nil => None,
                    _ => { return Err(mlua::Error::runtime(format!("Invalid type for build_env. Expected table, string, or nil: {:?}", build_env_val))); }
                };

                let actions: Vec<Action> = task_table.get("actions")?;
                let deps_opt: Option<DependencyList> = task_table.get("deps")?;
                let deps = deps_opt.map(|d| d.0).unwrap_or_default();
                let artifacts_opt: Option<Vec<Artifact>> = task_table.get("artifacts")?;
                let artifacts = artifacts_opt.unwrap_or_default();
        
                Ok(Task { name, build_env, actions, deps, artifacts })
            },
            mlua::Value::UserData(val) => Ok(val.borrow::<Task>()?.clone()),
            _ => Err(mlua::Error::runtime(format!("Unable to convert value to Task: {:?}", value)))
        }
    }
}
