use std::{
    fmt,
    path::{Component, Path, PathBuf}, sync::Arc
};

use crate::datamodel::{Action, Artifact, BuildEnv, Dependency, Project, ExternalTool, TaskDef};

#[derive(Debug)]
pub enum NameResolutionError {
    InvalidName(String),
    InvalidProjectName(String),
    PathToStringError(PathBuf),
    PathToNameError(PathBuf)
}

impl fmt::Display for NameResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NameResolutionError::*;
        match self {
            InvalidName(s) => write!(f, "Invalid name: {}", s),
            InvalidProjectName(s) => write!(f, "Invalid project name: {}", s),
            PathToStringError(p) => write!(f, "Error converting path to a UTF-8 string: {}", p.display()),
            PathToNameError(p) => write!(f, "Error converting path to a resource name: {}", p.display())
        }
    }
}

pub fn project_path_to_project_name(project_path: &Path) -> Result<String, NameResolutionError> {
    let mut project_name_components: Vec<String> = vec![String::from("")];
    for c in project_path.components() {
        match c {
            Component::CurDir => { /* do nothing */ },
            Component::Normal(s) => {
                let name_segment = s.to_str()
                    .ok_or_else(|| NameResolutionError::PathToStringError(project_path.to_owned()))?;
                project_name_components.push(name_segment.to_owned()); },
            _ => { return Err(NameResolutionError::PathToNameError(project_path.to_owned())); }
        }
    }

    let project_name = match project_name_components.len() {
        0 | 1 => String::from("/"),
        _ => project_name_components.join("/")
    };
    Ok(project_name)
}

pub fn resolve_path(project_path: &Path, path: &str) -> Result<Arc<str>, NameResolutionError> {
    let full_path = project_path.join(path);
    let full_path_str_opt = full_path.into_os_string().into_string();
    match full_path_str_opt {
        Ok(full_path_str) => {
            Ok(full_path_str.into())
        },
        Err(os_str) => Err(NameResolutionError::PathToStringError(PathBuf::from(os_str)))
    }
}

pub fn resolve_name(project_name: &str, name: &Arc<str>) -> Result<Arc<str>, NameResolutionError> {
    if name.starts_with("/") {
        return Ok(name.clone());
    }

    if !project_name.starts_with("/") {
        return Err(NameResolutionError::InvalidProjectName(project_name.to_owned()));
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

    let resolved_name = match full_name_segments.len() {
        0 | 1 => Arc::<str>::from("/"),
        _ => Arc::<str>::from(full_name_segments.join("/"))
    };
    Ok(resolved_name)
}

pub fn resolve_names_in_dependency(project_name: &str, project_path: &Path, dep: &mut Dependency) -> Result<(), NameResolutionError> {
    match dep {
        Dependency::File(f) => {
            *dep = Dependency::File(resolve_path(project_path, f.as_ref())?);
            Ok(())
        },
        Dependency::Task(t) => {
            *dep = Dependency::Task(resolve_name(project_name, &t)?);
            Ok(())
        },
        Dependency::Calc(c) => {
            *dep = Dependency::Calc(resolve_name(project_name, &c)?);
            Ok(())
        }
    }
}

fn resolve_names_in_action(project_name: &str, action: &mut Action) -> Result<(), NameResolutionError> {
    // Tool names are global, no need to resolve tool names
    
    for (_, env_name) in action.build_envs.iter_mut() {
        *env_name = resolve_name(project_name, &env_name)?;
    }

    Ok(())
}

fn resolve_names_in_build_env(project_name: &str, project_path: &Path, build_env: &mut BuildEnv) -> Result<(), NameResolutionError> {
    build_env.name = resolve_name(project_name, &build_env.name)?;

    for action in build_env.install.iter_mut() {
        resolve_names_in_action(project_name, action)?;
    }

    for dep in build_env.deps.iter_mut() {
        resolve_names_in_dependency(project_name, project_path, dep)?;
    }

    resolve_names_in_action(project_name, &mut build_env.action)?;

    Ok(())
}

fn resolve_names_in_tool(project_name: &str, tool: &mut ExternalTool) -> Result<(), NameResolutionError> {
    // External tool names are global, no need to resolve the name field
    if let Some(install) = &mut tool.install {
        resolve_names_in_action(project_name, install)?;
    }

    if let Some(check) = &mut tool.check {
        resolve_names_in_action(project_name, check)?;
    }

    resolve_names_in_action(project_name, &mut tool.action)?;

    Ok(())
}

fn resolve_names_in_artifact(project_path: &Path, artifact: &mut Artifact) -> Result<(), NameResolutionError> {
    artifact.filename = resolve_path(project_path, artifact.filename.as_ref())?;

    Ok(())
}

fn resolve_names_in_task(project_name: &str, project_path: &Path, task: &mut TaskDef) -> Result<(), NameResolutionError> {
    task.name = resolve_name(project_name, &task.name)?;

    if let Some((_, env_name)) = &mut task.build_env {
        *env_name = resolve_name(project_name, &env_name)?;
    }

    for action in task.actions.iter_mut() {
        resolve_names_in_action(project_name, action)?;
    }

    for dep in task.deps.iter_mut() {
        resolve_names_in_dependency(project_name, project_path, dep)?;
    }

    for artifact in task.artifacts.iter_mut() {
        resolve_names_in_artifact(project_path, artifact)?;
    }

    Ok(())
}

pub fn resolve_names_in_project(project: &mut Project) -> Result<(), NameResolutionError> {
    // Project name and path already fully-qualified relative to the workspace root

    for build_env in project.build_envs.iter_mut() {
        resolve_names_in_build_env(&project.name, &project.path, build_env)?;
    }

    for tool in project.tools.iter_mut() {
        resolve_names_in_tool(&project.name, tool)?;
    }

    for task in project.tasks.iter_mut() {
        resolve_names_in_task(&project.name, &project.path, task)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_name() {
        let full_name = resolve_name("/subproject", &Arc::<str>::from("myname")).unwrap();
        assert_eq!(full_name.as_ref(), "/subproject/myname");
    }

    #[test]
    fn test_resolve_name_from_root() {
        let full_name = resolve_name("/", &Arc::<str>::from("myname")).unwrap();
        assert_eq!(full_name.as_ref(), "/myname");
    }

    #[test]
    fn test_mutability() { 
        let mut val = String::from("hello");

        let vool = &mut val;

        *vool = (|v: &String| format!("{} there", v))(vool);

        assert_eq!(val, "hello there");
    }

}