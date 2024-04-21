use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf}
};

use crate::cobble::{
    datamodel::{Action, Artifact, BuildEnv, Dependency, ExternalTool, Task},
    workspace::{BuildUnitCollection, Project}
};

#[derive(Debug)]
pub enum NameResolutionError {
    InvalidProjectName(String),
    PathToStringError(PathBuf)
}

impl fmt::Display for NameResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NameResolutionError::*;
        match self {
            InvalidProjectName(s) => write!(f, "Invalid project name: {}", s),
            PathToStringError(p) => write!(f, "Error converting path to a UTF-8 string: {}", p.display())
        }
    }
}

fn resolve_path(project_path: &Path, path: &str) -> Result<String, NameResolutionError> {
    let full_path = project_path.join(path);
    let full_path_str_opt = full_path.to_str();
    match full_path_str_opt {
        Some(full_path_str) => {
            Ok(String::from(full_path_str))
        },
        None => Err(NameResolutionError::PathToStringError(full_path))
    }
}

fn resolve_name(project_name: &str, name: &str) -> Result<String, NameResolutionError> {
    if name.starts_with("/") {
        return Ok(String::from(name));
    }

    if !project_name.starts_with("/") {
        return Err(NameResolutionError::InvalidProjectName(String::from(project_name)));
    }

    let project_name_segments = project_name.split("/").filter(|s| s.len() > 0);
    let name_segments = name.split("/").filter(|s| s.len() > 0);

    let mut full_name_segments: Vec<&str> = vec![""].into_iter()
        .chain(project_name_segments)
        .chain(name_segments)
        .collect();

    let mut idx = 0;
    while idx < full_name_segments.len() {
        match full_name_segments[idx] {
            ".." => {
                full_name_segments.remove(idx);
                // Ignore ".." segments that would go past the root
                if idx > 1 {
                    full_name_segments.remove(idx-1);
                }
            },
            "." => {
                full_name_segments.remove(idx);
            },
            _ => {
                idx += 1;
            }
        }
    }

    Ok(full_name_segments.join("/"))
}

fn resolve_names_in_dependency(project: &Project, dep: &Dependency) -> Result<Dependency, NameResolutionError> {
    match dep {
        Dependency::File(f) => {
            let full_path = resolve_path(project.path.as_path(), f.as_str())?;
            Ok(Dependency::File(full_path))
        },
        Dependency::Task(t) => {
            let full_task_name = resolve_name(project.name.as_str(), t.as_str())?;
            Ok(Dependency::Task(full_task_name))
        }
    }
}

fn resolve_names_in_action(project: &Project, action: &Action) -> Result<Action, NameResolutionError> {
    // Tool names are global, no need to resolve
    let tools = action.tools.clone();
    
    let mut build_envs: HashMap<String, String> = HashMap::new();
    for (env_alias, env_name) in action.build_envs.iter() {
        let full_build_env_name = resolve_name(project.name.as_str(), env_name.as_str())?;
        build_envs.insert(env_alias.clone(), full_build_env_name);
    }

    let cmd = action.cmd.clone();

    Ok(Action{ tools, build_envs, cmd })
}

fn resolve_names_in_build_env(project: &Project, build_env: &BuildEnv) -> Result<BuildEnv, NameResolutionError> {
    let name = resolve_name(project.name.as_str(), build_env.name.as_str())?;

    let mut install: Vec<Action> = Vec::new();
    for action in build_env.install.iter() {
        install.push(resolve_names_in_action(project, action)?);
    }

    let mut deps: Vec<Dependency> = Vec::new();
    for dep in build_env.deps.iter() {
        deps.push(resolve_names_in_dependency(project, dep)?);
    }

    let action = resolve_names_in_action(project, &build_env.action)?;

    Ok(BuildEnv{ name, install, deps, action })
}

fn resolve_names_in_tool(project: &Project, tool: &ExternalTool) -> Result<ExternalTool, NameResolutionError> {
    // External tool names are global, no need to resolve
    let name = tool.name.clone();

    let install = tool.install.as_ref()
        .map(|a| resolve_names_in_action(project, a))
        .transpose()?;

    let check = tool.check.as_ref()
        .map(|a| resolve_names_in_action(project, a))
        .transpose()?;

    let action = resolve_names_in_action(project, &tool.action)?;

    Ok(ExternalTool{ name, install, check, action })
}

fn resolve_names_in_artifact(project: &Project, artifact: &Artifact) -> Result<Artifact, NameResolutionError> {
    let filename = resolve_path(project.path.as_path(), artifact.filename.as_str())?;

    Ok(Artifact{ filename })
}

fn resolve_names_in_task(project: &Project, task: &Task) -> Result<Task, NameResolutionError> {
    let name = resolve_name(project.name.as_str(), task.name.as_str())?;

    let build_env = match task.build_env.as_ref() {
        Some((env_alias, env_name)) => {
            let resolved_env_name = resolve_name(project.name.as_str(), env_name.as_str())?;
            Some((env_alias.clone(), resolved_env_name))
        },
        None => None
    };

    let mut actions: Vec<Action> = Vec::new();
    for action in task.actions.iter() {
        actions.push(resolve_names_in_action(project, action)?);
    }

    let mut deps: Vec<Dependency> = Vec::new();
    for dep in task.deps.iter() {
        deps.push(resolve_names_in_dependency(project, dep)?);
    }

    let mut artifacts: Vec<Artifact> = Vec::new();
    for artifact in task.artifacts.iter() {
        artifacts.push(resolve_names_in_artifact(project, artifact)?);
    }

    Ok(Task{ name, build_env, actions, deps, artifacts })
}

pub fn resolve_names_in_build_units<'a, P>(projects: P) -> Result<BuildUnitCollection, NameResolutionError>
    where P: Iterator<Item = &'a Project>
{
    let mut build_units = BuildUnitCollection {
        build_envs: Vec::new(),
        tools: Vec::new(),
        tasks: Vec::new()
    };

    for project in projects {
        for build_env in project.build_envs.iter() {
            build_units.build_envs.push(resolve_names_in_build_env(project, build_env)?);
        }

        for tool in project.tools.iter() {
            build_units.tools.push(resolve_names_in_tool(project, tool)?);
        }

        for task in project.tasks.iter() {
            build_units.tasks.push(resolve_names_in_task(project, task)?);
        }
    }    

    Ok(build_units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_name() {
        let full_name = resolve_name("/subproject", "myname").unwrap();
        assert_eq!(full_name, "/subproject/myname");
    }

    #[test]
    fn test_resolve_name_from_root() {
        let full_name = resolve_name("/", "myname").unwrap();
        assert_eq!(full_name, "/myname");
    }

}