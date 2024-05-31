use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use crate::project_def::{BuildEnvDef, ExternalTool, TaskDef};

#[derive(Debug)]
pub struct Project {
    pub name: Arc<str>,
    pub path: Arc<Path>,
    pub build_envs: Vec<BuildEnvDef>,
    pub tasks: Vec<TaskDef>,
    pub tools: Vec<ExternalTool>,
    pub child_project_names: Vec<Arc<str>>,
    pub project_source_deps: Vec<Arc<str>>,
}

impl fmt::Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Project(")?;
        write!(f, "name=\"{}\",", &self.name)?;
        write!(f, "path={},", self.path.display())?;

        f.write_str("build_envs=[")?;
        for (i, build_env) in self.build_envs.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?
            }
            write!(f, "{}", build_env)?;
        }
        f.write_str("], ")?;

        f.write_str("tasks=[")?;
        for (i, task) in self.tasks.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", task)?;
        }
        f.write_str("], ")?;

        f.write_str("tools=[")?;
        for (i, tool) in self.tools.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", tool)?;
        }
        f.write_str("],")?;

        f.write_str("child_projects=[")?;
        for (i, proj) in self.child_project_names.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", proj)?;
        }
        f.write_str("],")?;

        f.write_str("project_file_deps=[")?;
        for (i, proj) in self.project_source_deps.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", proj)?;
        }
        f.write_str("],")?;

        f.write_str(")")
    }
}

impl<'lua> mlua::FromLua<'lua> for Project {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let project_table = match value {
            mlua::Value::Table(tbl) => tbl,
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "Project must be a lua table value"
                )));
            }
        };

        let name_str: String = project_table.get("name")?;
        let name = Arc::<str>::from(name_str);

        let path_str: String = project_table.get("dir")?;
        let path_buf = PathBuf::from_str(path_str.as_str())
            .expect("Conversion from str to PathBuf is infalliable");
        let path = Arc::<Path>::from(path_buf);

        let build_envs: Vec<BuildEnvDef> = project_table.get("build_envs")?;
        let tasks: Vec<TaskDef> = project_table.get("tasks")?;
        let tools: Vec<ExternalTool> = project_table.get("tools")?;

        let child_projects: Vec<mlua::Table> = project_table.get("child_projects")?;
        let mut child_project_names: Vec<Arc<str>> = Vec::with_capacity(child_projects.len());
        for child_project in child_projects {
            let child_project_name: String = child_project.get("name")?;
            child_project_names.push(child_project_name.into());
        }

        let project_source_deps_strvec: Vec<String> = project_table.get("project_source_deps")?;
        let project_source_deps: Vec<Arc<str>> = project_source_deps_strvec
            .into_iter()
            .map(|s| s.into())
            .collect();

        Ok(Project {
            name,
            path,
            build_envs,
            tasks,
            tools,
            child_project_names,
            project_source_deps,
        })
    }
}
