use std::{borrow::Cow, collections::{HashMap, HashSet}, convert::AsRef, fmt, sync::Arc};

use crate::{datamodel::Project, workspace::{execute::{TaskExecutionError, TaskExecutor}, query::{Task, Workspace}}};

#[derive(Debug)]
pub enum ExecutionGraphError {
    CycleError(String),
    TaskLookupError(String),
    DuplicateFileProviderError{provider1: String, provider2: String, file: String},
    TaskExecutionError(TaskExecutionError)
}

impl fmt::Display for ExecutionGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecutionGraphError::*;
        match self {
            CycleError(task) => write!(f, "Cycle detected at {}", task),
            TaskLookupError(task) => write!(f, "Task not found: {}", task),
            DuplicateFileProviderError{provider1, provider2, file} =>
                write!(f, "Multiple providers found for file dependency {}: {}, {}", file, provider1, provider2),
            TaskExecutionError(e) => write!(f, "{}", e)
        }
    }
}

pub fn compute_file_providers<'a, P>(projects: P) -> HashMap<&'a str, &'a str>
    where P: Iterator<Item = &'a Project>
{
    let mut file_providers: HashMap<&'a str, &'a str> = HashMap::new();

    for project in projects {
        for task in project.tasks.iter() {
            for artifact in task.artifacts.iter() {
                file_providers.insert(artifact.filename.as_str(), task.name.as_str());
            }
        }
    }

    file_providers
}

fn resolve_calculated_dependencies_in_subtree<'a>(task_name: &str, file_providers: &HashMap<&'a str, &'a str>, workspace: &'a mut Workspace, visited: &mut HashSet<String>, task_executor: &mut TaskExecutor) -> Result<bool, ExecutionGraphError> {
    if visited.contains(task_name) {
        return Err(ExecutionGraphError::CycleError(task_name.to_owned()));
    }

    visited.insert(task_name.to_owned());
    
    let task = workspace.tasks.get(task_name)
        .ok_or_else(|| ExecutionGraphError::TaskLookupError(task_name.to_owned()))?
        .clone();


    for calc_dep in task.calc_deps.iter() {
        resolve_calculated_dependencies_in_subtree(calc_dep.as_str(), file_providers, workspace, visited, task_executor)?;

        task_executor.execute_tasks(workspace, Some(calc_dep.as_str()).iter().copied())
            .map_err(|e| ExecutionGraphError::TaskExecutionError(e))?;


        let executor_cache = task_executor.cache();
        let task_outputs = executor_cache.task_outputs.read().unwrap();
        let task_output = task_outputs.get(calc_dep)
            .expect("calculated dependency task output should be available after executing");

        println!("Calculated dependencies: {}", task_output);
    }

    for dep in task.task_deps.iter() {
        resolve_calculated_dependencies_in_subtree(dep.as_str(), file_providers, workspace, visited, task_executor)?;
    }

    visited.remove(task_name);

    Ok(false)
}


#[cfg(test)]
mod tests {
   
}
