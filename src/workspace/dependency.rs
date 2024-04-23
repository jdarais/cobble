use std::{collections::HashMap, convert::AsRef, fmt, hash::Hash, path::Path};

use crate::{datamodel::Project, workspace::query::WorkspaceTargetRef};

#[derive(Debug)]
pub enum CalculateDependenciesError {
    DuplicateProvider{ task_1: String, task_2: String, file: String}
}

impl fmt::Display for CalculateDependenciesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CalculateDependenciesError::*;
        match self {
            DuplicateProvider{task_1, task_2, file} => write!(f, "Encountered two projects ({}, {}) claiming the same artifact: {}", task_1, task_2, file)
        }
    }
}

pub struct TargetDependencyInfo<'a> {
    pub file_providers: HashMap<&'a Path, &'a str>,
    pub task_dependencies: HashMap<&'a str, Vec<TargetDependency<'a>>>
}

pub enum TargetDependency<'a> {
    Task(&'a str),
    File(&'a Path),
    BuildEnv(&'a str),
    ExternalTool(&'a str)
}

pub fn create_file_providers_map<'a, T>(targets: T) -> Result<HashMap<&'a str, &'a str>, CalculateDependenciesError>
    where T: Iterator<Item = WorkspaceTargetRef<'a>>
{
    let mut file_providers: HashMap<&'a str, &'a str> = HashMap::new();

    for target in targets {
        if let WorkspaceTargetRef::Task(task) = target {
            for artifact in task.artifacts.iter() {
                if file_providers.contains_key(artifact.filename.as_str()) {
                    return Err(CalculateDependenciesError::DuplicateProvider {
                        task_1: String::from(file_providers[artifact.filename.as_str()]),
                        task_2: task.name.clone(),
                        file: artifact.filename.clone()
                    });
                }
                file_providers.insert(artifact.filename.as_str(), task.name.as_str());
            }
        }
    }

    Ok(file_providers)
}
