// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::env::current_dir;
use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use crate::project_def::build_env::EnvSetupTask;
use crate::project_def::{
    Action, Artifacts, BuildEnvDef, Dependencies, ExternalTool, Project, TaskDef,
};

#[derive(Debug)]
pub enum NameResolutionError {
    InvalidName(String),
    PathNotInWorkspace(String),
    InvalidProjectName(String),
    PathToStringError(PathBuf),
    PathToNameError(PathBuf),
}

impl Error for NameResolutionError {}

impl fmt::Display for NameResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use NameResolutionError::*;
        match self {
            InvalidName(s) => write!(f, "Invalid name: {}", s),
            PathNotInWorkspace(s) => write!(f, "Path is not in the workspace: {}", s),
            InvalidProjectName(s) => write!(f, "Invalid project name: {}", s),
            PathToStringError(p) => write!(
                f,
                "Error converting path to a UTF-8 string: {}",
                p.display()
            ),
            PathToNameError(p) => write!(
                f,
                "Error converting path to a resource name: {}",
                p.display()
            ),
        }
    }
}

pub fn path_relative_to_workspace_dir(path: &Path) -> Result<PathBuf, NameResolutionError> {
    if path.is_relative() {
        return Ok(PathBuf::from(path));
    }

    // Here we assume that cwd is the workspace root
    let cwd = current_dir().expect("current working directory must be set and exist");

    path.strip_prefix(cwd)
        .map(|p| PathBuf::from(p))
        .map_err(|_e| NameResolutionError::PathNotInWorkspace(path.to_string_lossy().to_string()))
}

pub fn project_path_to_project_name(project_path: &Path) -> Result<String, NameResolutionError> {
    let mut project_name_components: Vec<String> = vec![String::from("")];
    for c in project_path.components() {
        match c {
            Component::CurDir => { /* do nothing */ }
            Component::ParentDir => {
                if project_name_components.len() > 0 {
                    project_name_components.pop();
                }
            }
            Component::Normal(s) => {
                let name_segment = s.to_str().ok_or_else(|| {
                    NameResolutionError::PathToStringError(project_path.to_owned())
                })?;
                project_name_components.push(name_segment.to_owned());
            }
            _ => {
                return Err(NameResolutionError::PathToNameError(
                    project_path.to_owned(),
                ));
            }
        }
    }

    let project_name = match project_name_components.len() {
        0 | 1 => String::from(""),
        _ => project_name_components.join("/"),
    };
    Ok(project_name)
}

pub fn resolve_path(project_path: &Path, path: &str) -> Result<Arc<str>, NameResolutionError> {
    use Component::*;

    let mut path_components: Vec<Component> = Vec::new();
    let joined_path = project_path.join(path);
    for comp in joined_path.components() {
        match comp {
            Prefix(pre) => { path_components.push(Prefix(pre)); }
            RootDir => { path_components.push(RootDir); }
            CurDir => { /* skip */ }
            Normal(comp_str) => { path_components.push(Normal(comp_str)); }
            ParentDir => match path_components.pop() {
                Some(parent_comp) => match parent_comp {
                    Prefix(_) | RootDir => {
                        path_components.push(parent_comp);
                    }
                    ParentDir => {
                        path_components.push(ParentDir);
                        path_components.push(ParentDir);
                    }
                    Normal(_) => { /* ParentDir negates a Normal parent component */ }
                    CurDir => { panic!("CurDir should never have been added to path_components"); }
                },
                None => {
                    // There was nothing to pop, so this ".." component is at the beginning of the path, and we should leave it there
                    path_components.push(ParentDir);
                }
            }
        }
    }
    let full_path = PathBuf::from_iter(path_components.into_iter());
    let full_path_str_opt = full_path.into_os_string().into_string();
    match full_path_str_opt {
        Ok(full_path_str) => Ok(full_path_str.into()),
        Err(os_str) => Err(NameResolutionError::PathToStringError(PathBuf::from(
            os_str,
        ))),
    }
}

fn canonicalize_name(mut full_name_segments: Vec<&str>) -> Arc<str> {
    let mut idx = 0;
    while idx < full_name_segments.len() {
        match full_name_segments[idx] {
            ".." => {
                full_name_segments.remove(idx);
                // Ignore ".." segments that would go past the root
                if idx > 0 {
                    full_name_segments.remove(idx - 1);
                    idx -= 1;
                }
            }
            "." => {
                full_name_segments.remove(idx);
            }
            _ => {
                if full_name_segments[idx].len() == 0 {
                    full_name_segments.remove(idx);
                } else {
                    idx += 1;
                }
            }
        }
    }

    match full_name_segments.len() {
        0 => Arc::<str>::from("/"),
        _ => {
            let mut full_name = String::from("/");
            let joined_path = full_name_segments.join("/");
            full_name.push_str(joined_path.as_str());
            full_name.into()
        },
    }
}

pub fn resolve_name(project_name: &str, project_path: &Path, name: &Arc<str>) -> Result<Arc<str>, NameResolutionError> {
    if name.starts_with("/") {
        return Ok(canonicalize_name(name.split("/").collect()));
    }


    if name.starts_with("[") {
        let path_prefix_end_index = name.find("]");
        let path_prefix = match path_prefix_end_index {
            None => { return Err(NameResolutionError::InvalidName(String::from(name.as_ref()))); },
            Some(idx) => &name[1..idx]
        };

        let path_prefix_rel_to_workspace = path_relative_to_workspace_dir(&project_path.join(Path::new(path_prefix)))?;
        let mut resolved_name = project_path_to_project_name(&path_prefix_rel_to_workspace)?;
        resolved_name.push_str(&name[path_prefix.len()+2..]);
        return Ok(canonicalize_name(resolved_name.split("/").collect()));
    }

    if !project_name.starts_with("/") {
        return Err(NameResolutionError::InvalidProjectName(
            project_name.to_owned(),
        ));
    }

    let project_name_segments = project_name.split("/").filter(|s| s.len() > 0);
    let name_segments = name.split("/").filter(|s| s.len() > 0);

    let full_name_segments: Vec<&str> =vec![""]
        .into_iter()
        .chain(project_name_segments)
        .chain(name_segments)
        .collect();

    Ok(canonicalize_name(full_name_segments))
}

pub fn resolve_names_in_dependency_list(
    project_name: &str,
    project_path: &Path,
    deps: &mut Dependencies,
) -> Result<(), NameResolutionError> {
    for (_, f_path) in deps.files.iter_mut() {
        *f_path = resolve_path(project_path, f_path.as_ref())?
    }

    for (_, t_name) in deps.tasks.iter_mut() {
        *t_name = resolve_name(project_name, project_path, t_name)?;
    }

    for c_name in deps.calc.iter_mut() {
        *c_name = resolve_name(project_name, project_path, c_name)?;
    }

    Ok(())
}

fn resolve_names_in_action(
    project_name: &str,
    project_path: &Path,
    action: &mut Action,
) -> Result<(), NameResolutionError> {
    // Tool names are global, no need to resolve tool names

    for (_, env_name) in action.build_envs.iter_mut() {
        *env_name = resolve_name(project_name, project_path, &env_name)?;
    }

    Ok(())
}

fn resolve_names_in_build_env(
    project_name: &str,
    project_path: &Path,
    build_env: &mut BuildEnvDef,
) -> Result<(), NameResolutionError> {
    build_env.name = resolve_name(project_name, project_path, &build_env.name)?;
    
    if let Some(setup_task) = &mut build_env.setup_task {
        match setup_task {
            EnvSetupTask::Ref(name) => { *name = resolve_name(project_name, project_path, &name.clone())?; }
            EnvSetupTask::Inline(task) => { resolve_names_in_task(project_name, project_path, task)?; }
        }
    }

    resolve_names_in_action(project_name, project_path, &mut build_env.action)?;

    Ok(())
}

fn resolve_names_in_tool(
    project_name: &str,
    project_path: &Path,
    tool: &mut ExternalTool,
) -> Result<(), NameResolutionError> {
    // External tool names are global, no need to resolve the name field
    if let Some(install) = &mut tool.install {
        resolve_names_in_action(project_name, project_path, install)?;
    }

    if let Some(check) = &mut tool.check {
        resolve_names_in_action(project_name, project_path, check)?;
    }

    resolve_names_in_action(project_name, project_path, &mut tool.action)?;

    Ok(())
}

fn resolve_names_in_artifacts(
    project_name: &str,
    project_path: &Path,
    artifacts: &mut Artifacts,
) -> Result<(), NameResolutionError> {
    // artifact.filename = resolve_path(project_path, artifact.filename.as_ref())?;
    for f in artifacts.files.iter_mut() {
        *f = resolve_path(project_path, f.as_ref())?;
    }

    for c in artifacts.calc.iter_mut() {
        *c = resolve_name(project_name, project_path, &c)?;
    }

    Ok(())
}

fn resolve_names_in_task(
    project_name: &str,
    project_path: &Path,
    task: &mut TaskDef,
) -> Result<(), NameResolutionError> {
    task.name = resolve_name(project_name, project_path, &task.name)?;

    if let Some((_, env_name)) = &mut task.build_env {
        *env_name = resolve_name(project_name, project_path, &env_name)?;
    }

    for action in task.actions.iter_mut() {
        resolve_names_in_action(project_name, project_path, action)?;
    }

    resolve_names_in_dependency_list(project_name, project_path, &mut task.deps)?;

    resolve_names_in_artifacts(project_name, project_path, &mut task.artifacts)?;

    Ok(())
}

pub fn resolve_names_in_project(project: &mut Project) -> Result<(), NameResolutionError> {
    // Project name and path already fully-qualified relative to the workspace root

    for build_env in project.build_envs.iter_mut() {
        resolve_names_in_build_env(&project.name, &project.path, build_env)?;
    }

    for tool in project.tools.iter_mut() {
        resolve_names_in_tool(&project.name, &project.path, tool)?;
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
        let full_name = resolve_name("/subproject", &Path::new(".").join("subproject"), &Arc::<str>::from("myname")).unwrap();
        assert_eq!(full_name.as_ref(), "/subproject/myname");
    }

    #[test]
    fn test_resolve_name_with_path_prefix() {
        let sep = std::path::MAIN_SEPARATOR;
        let name = format!("[..{sep}otherproject]/myname");
        let full_name = resolve_name("/subproject", &Path::new(".").join("subproject"), &Arc::<str>::from(name)).unwrap();
        assert_eq!(full_name.as_ref(), "/otherproject/myname");
    }

    #[test]
    fn test_resolve_name_from_root() {
        let full_name = resolve_name("/", Path::new("."), &Arc::<str>::from("myname")).unwrap();
        assert_eq!(full_name.as_ref(), "/myname");
    }

    #[test]
    fn test_resolve_path_removes_curdir_components() {
        let sep = std::path::MAIN_SEPARATOR;
        let resolved_path = resolve_path(Path::new("./a/test/path"), "./a/./path/./with/./dots").unwrap();
        assert_eq!(resolved_path.as_ref(), format!("a{sep}test{sep}path{sep}a{sep}path{sep}with{sep}dots").as_str());
    }

    #[test]
    fn test_resolve_path_resolves_parent_dir_components() {
        let sep = std::path::MAIN_SEPARATOR;
        let resolved_path = resolve_path(Path::new("./a/test/path"), "../../path/in/other/dir").unwrap();
        assert_eq!(resolved_path.as_ref(), format!("a{sep}path{sep}in{sep}other{sep}dir").as_str());
    }

    #[test]
    fn test_resolve_path_leaves_parent_dir_at_start_of_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let resolved_path = resolve_path(Path::new("./a"), "../../path/in/other/dir").unwrap();
        assert_eq!(resolved_path.as_ref(), format!("..{sep}path{sep}in{sep}other{sep}dir").as_str());
    }

    #[test]
    fn test_resolve_path_remove_parent_dir_component_at_root_of_abs_path() {
        let sep = std::path::MAIN_SEPARATOR;
        let resolved_path = resolve_path(Path::new("/a"), "../../path/in/other/dir").unwrap();
        assert_eq!(resolved_path.as_ref(), format!("{sep}path{sep}in{sep}other{sep}dir").as_str());
    }
}
