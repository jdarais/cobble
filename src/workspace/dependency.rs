use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::datamodel::{DependencyList, Project};
use crate::workspace::execute::{TaskExecutionError, TaskExecutor};
use crate::workspace::graph::{add_dependency_to_task, Task, Workspace};
use crate::workspace::resolve::{resolve_names_in_dependency, NameResolutionError};

#[derive(Debug)]
pub enum ExecutionGraphError {
    CycleError(String),
    TaskLookupError(String),
    DuplicateFileProviderError{provider1: String, provider2: String, file: String},
    TaskExecutionError(TaskExecutionError),
    OutputDeserializationError(String),
    NameResolutionError(NameResolutionError)
}

impl fmt::Display for ExecutionGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecutionGraphError::*;
        match self {
            CycleError(task) => write!(f, "Cycle detected at {}", task),
            TaskLookupError(task) => write!(f, "Task not found: {}", task),
            DuplicateFileProviderError{provider1, provider2, file} =>
                write!(f, "Multiple providers found for file dependency {}: {}, {}", file, provider1, provider2),
            TaskExecutionError(e) => write!(f, "{}", e),
            OutputDeserializationError(s) => write!(f, "Error reading output of dependency calc task: {}", s),
            NameResolutionError(e) => write!(f, "{}", e)
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

pub fn resolve_calculated_dependencies_in_subtree<'a>(task_name: &str, file_providers: &HashMap<&'a str, &'a str>, workspace: &'a mut Workspace, task_executor: &mut TaskExecutor) -> Result<bool, ExecutionGraphError> {
    resolve_calculated_dependencies_in_subtree_with_history(task_name, file_providers, workspace, &mut HashSet::new(), task_executor)
}

fn resolve_calculated_dependencies_in_subtree_with_history<'a>(task_name: &str, file_providers: &HashMap<&'a str, &'a str>, workspace: &'a mut Workspace, visited: &mut HashSet<String>, task_executor: &mut TaskExecutor) -> Result<bool, ExecutionGraphError> {
    if visited.contains(task_name) {
        return Err(ExecutionGraphError::CycleError(task_name.to_owned()));
    }

    visited.insert(task_name.to_owned());
    
    let task = workspace.tasks.get(task_name)
        .ok_or_else(|| ExecutionGraphError::TaskLookupError(task_name.to_owned()))?
        .clone();

    let mut task_cow: Cow<Task> = Cow::Borrowed(task.as_ref());

    for calc_dep in task.calc_deps.iter() {
        resolve_calculated_dependencies_in_subtree_with_history(calc_dep.as_str(), file_providers, workspace, visited, task_executor)?;

        task_executor.execute_tasks(workspace, Some(calc_dep.as_str()).iter().copied())
            .map_err(|e| ExecutionGraphError::TaskExecutionError(e))?;


        let executor_cache = task_executor.cache();
        let task_outputs = executor_cache.task_outputs.read().unwrap();
        let task_output = task_outputs.get(calc_dep)
            .expect("calculated dependency task output should be available after executing");

        let deps = DependencyList::from_json(task_output.clone())
            .map_err(|e| ExecutionGraphError::OutputDeserializationError(e.to_string()))?;

        task_cow.to_mut().calc_deps = task_cow.to_mut().calc_deps.drain(..).filter(|s| s != calc_dep).collect();
        for mut dep in deps.0.into_iter() {
            resolve_names_in_dependency(task.project_name.as_str(), task.dir.as_path(), &mut dep)
                .map_err(|e| ExecutionGraphError::NameResolutionError(e))?;
            add_dependency_to_task(&dep, file_providers, task_cow.to_mut());
        }
    }

    if let Cow::Owned(updated_task) = task_cow {
        workspace.tasks.insert(task_name.to_owned(), Arc::new(updated_task));
    }

    for dep in task.task_deps.iter() {
        resolve_calculated_dependencies_in_subtree_with_history(dep.as_str(), file_providers, workspace, visited, task_executor)?;
    }

    visited.remove(task_name);

    Ok(false)
}


#[cfg(test)]
mod tests {
   
}
