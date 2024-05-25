use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::Arc;

use crate::project_def::dependency::Dependencies;
use crate::project_def::{DependencyListByType, Project};
use crate::execute::execute::{TaskExecutionError, TaskExecutor};
use crate::workspace::{add_dependency_list_to_task, Task, Workspace};
use crate::resolve::{resolve_names_in_dependency_list, NameResolutionError};

#[derive(Debug)]
pub enum ExecutionGraphError {
    CycleError(Arc<str>),
    TaskLookupError(Arc<str>),
    DuplicateFileProviderError {
        provider1: String,
        provider2: String,
        file: String,
    },
    TaskExecutionError(TaskExecutionError),
    OutputDeserializationError(String),
    NameResolutionError(NameResolutionError),
}

impl Error for ExecutionGraphError {}
impl fmt::Display for ExecutionGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecutionGraphError::*;
        match self {
            CycleError(task) => write!(f, "Cycle detected at {}", task),
            TaskLookupError(task) => write!(f, "Task not found: {}", task),
            DuplicateFileProviderError {
                provider1,
                provider2,
                file,
            } => write!(
                f,
                "Multiple providers found for file dependency {}: {}, {}",
                file, provider1, provider2
            ),
            TaskExecutionError(e) => write!(f, "{}", e),
            OutputDeserializationError(s) => {
                write!(f, "Error reading output of dependency calc task: {}", s)
            }
            NameResolutionError(e) => write!(f, "{}", e),
        }
    }
}

pub fn compute_file_providers<'a, P>(projects: P) -> HashMap<Arc<str>, Arc<str>>
where
    P: Iterator<Item = &'a Project>,
{
    let mut file_providers: HashMap<Arc<str>, Arc<str>> = HashMap::new();

    for project in projects {
        for task in project.tasks.iter() {
            for artifact in task.artifacts.iter() {
                file_providers.insert(artifact.filename.clone(), task.name.clone());
            }
        }
    }

    file_providers
}

pub fn resolve_calculated_dependencies_in_subtrees<'a, T>(
    task_names: T,
    workspace: &mut Workspace,
    task_executor: &mut TaskExecutor,
) -> Result<(), ExecutionGraphError>
where
    T: Iterator<Item = &'a Arc<str>>,
{
    for task_name in task_names {
        // TODO: Track the names of tasks that have get visited with each invocation so we can skip
        // them if they show up in another subtree
        resolve_calculated_dependencies_in_subtree(task_name, workspace, task_executor)?;
    }
    Ok(())
}

pub fn resolve_calculated_dependencies_in_subtree(
    task_name: &Arc<str>,
    workspace: &mut Workspace,
    task_executor: &mut TaskExecutor,
) -> Result<(), ExecutionGraphError> {
    let mut changed = true;
    while changed {
        changed = resolve_calculated_dependencies_in_subtree_once_with_history(
            task_name,
            workspace,
            &mut HashSet::new(),
            task_executor,
        )?;
    }
    Ok(())
}

fn resolve_calculated_dependencies_in_subtree_once_with_history(
    task_name: &Arc<str>,
    workspace: &mut Workspace,
    visited: &mut HashSet<Arc<str>>,
    task_executor: &mut TaskExecutor,
) -> Result<bool, ExecutionGraphError> {
    if visited.contains(task_name) {
        return Err(ExecutionGraphError::CycleError(task_name.clone()));
    }

    let mut changed = false;

    visited.insert(task_name.to_owned());

    let task = workspace
        .tasks
        .get(task_name)
        .ok_or_else(|| ExecutionGraphError::TaskLookupError(task_name.to_owned()))?
        .clone();

    let mut task_cow: Cow<Task> = Cow::Borrowed(task.as_ref());

    for calc_dep in task.calc_deps.iter() {
        resolve_calculated_dependencies_in_subtree_once_with_history(
            &calc_dep,
            workspace,
            visited,
            task_executor,
        )?;

        task_executor
            .execute_tasks(workspace, Some(calc_dep.clone()).iter())
            .map_err(|e| ExecutionGraphError::TaskExecutionError(e))?;

        let executor_cache = task_executor.cache();
        let task_outputs = executor_cache.task_outputs.read().unwrap();
        let task_output = task_outputs
            .get(calc_dep.as_ref())
            .expect("calculated dependency task output should be available after executing");

        let deps_list_by_type: DependencyListByType =
            serde_json::from_value(task_output.clone())
                .map_err(|e| ExecutionGraphError::OutputDeserializationError(e.to_string()))?;
        let mut deps: Dependencies = deps_list_by_type.into();

        // Do not allow calculated dependencies to produce more calculated dependencies for the same task
        deps.calc.clear();

        task_cow.to_mut().calc_deps = task_cow
            .to_mut()
            .calc_deps
            .drain(..)
            .filter(|s| s != calc_dep)
            .collect();

        resolve_names_in_dependency_list(task.project_name.as_ref(), task.dir.as_ref(), &mut deps)
            .map_err(|e| ExecutionGraphError::NameResolutionError(e))?;
        add_dependency_list_to_task(&deps, &workspace.file_providers, task_cow.to_mut());

        changed = true;
    }

    if let Cow::Owned(updated_task) = task_cow {
        println!("Replacing task {}: {:?}", task_name, updated_task);
        workspace
            .tasks
            .insert(task_name.clone(), Arc::new(updated_task));
    }

    // TODO: Extract logic for getting all task dependencies, (including file providers for file dependencies) into a shared function
    for dep in task.task_deps.values() {
        changed = changed
            || resolve_calculated_dependencies_in_subtree_once_with_history(
                dep,
                workspace,
                visited,
                task_executor,
            )?;
    }

    for f_dep in task.file_deps.values() {
        if let Some(t_dep) = &f_dep.provided_by_task {
            changed = changed
            || resolve_calculated_dependencies_in_subtree_once_with_history(
                t_dep,
                workspace,
                visited,
                task_executor,
            )?;
        }
    }

    visited.remove(task_name);

    Ok(changed)
}

#[cfg(test)]
mod tests {}
