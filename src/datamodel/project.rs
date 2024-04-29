use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use crate::datamodel::{BuildEnv, ExternalTool, TaskDef};


#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub build_envs: Vec<BuildEnv>,
    pub tasks: Vec<TaskDef>,
    pub tools: Vec<ExternalTool>,
    pub child_project_names: Vec<String>
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Project(")?;
        write!(f, "name=\"{}\",", &self.name)?;
        write!(f, "path={},", self.path.display())?;
        
        f.write_str("build_envs=[")?;
        for (i, build_env) in self.build_envs.iter().enumerate() {
            if i > 0 { f.write_str(", ")? }
            write!(f, "{}", build_env)?;
        }
        f.write_str("], ")?;

        f.write_str("tasks=[")?;
        for (i, task) in self.tasks.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", task)?;
        }
        f.write_str("], ")?;

        f.write_str("tools=[")?;
        for (i, tool) in self.tools.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", tool)?;
        }
        f.write_str("]")?;

        f.write_str(")")
    }
}

impl <'lua> mlua::FromLua<'lua> for Project {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let project_table = match value {
            mlua::Value::Table(tbl) => tbl,
            _ => { return Err(mlua::Error::runtime(format!("Project must be a lua table value"))); }
        };

        let name: String = project_table.get("name")?;
        let path_str: String = project_table.get("dir")?;
        let path = PathBuf::from_str(path_str.as_str()).expect("Conversion from str to PathBuf is infalliable");
        let build_envs: Vec<BuildEnv> = project_table.get("build_envs")?;
        let tasks: Vec<TaskDef> = project_table.get("tasks")?;
        let tools: Vec<ExternalTool> = project_table.get("tools")?;

        let child_projects: Vec<mlua::Table> = project_table.get("child_projects")?;
        let mut child_project_names: Vec<String> = Vec::with_capacity(child_projects.len());
        for child_project in child_projects {
            child_project_names.push(child_project.get("name")?);
        }

        Ok(Project{ name, path, build_envs, tasks, tools, child_project_names })
    }
}