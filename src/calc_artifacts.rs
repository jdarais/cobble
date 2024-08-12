use std::{collections::HashMap, error::Error, fmt, sync::Arc};

use crate::{dependency::{resolve_calculated_dependencies_in_subtrees, ExecutionGraphError}, execute::execute::{TaskExecutionError, TaskExecutor}, resolve::{resolve_path, NameResolutionError}, workspace::{Task, Workspace}};

#[derive(Debug)]
pub enum CalcArtifactsError {
    DependencyError(ExecutionGraphError),
    ExecutionError(TaskExecutionError),
    OutputError{ task_name: Arc<str>, task_output: serde_json::Value, error: serde_json::Error },
    NameResolutionError{ task_name: Arc<str>, path: String, error: NameResolutionError }
}

impl fmt::Display for CalcArtifactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CalcArtifactsError::*;
        match self {
            DependencyError(e) => write!(f, "Error resolving calculated dependencies in calc artifacts tasks: {e}"),
            ExecutionError(e) => write!(f, "Error while executing artifacts task or one of its dependencies: {e}"),
            OutputError{ task_name, task_output, error } => write!(f, "Error in output of calc artifacts task {task_name}: output={task_output}, error={error}"),
            NameResolutionError{ task_name, path, error } => write!(f, "Error resolving calc artifact path returned by task {task_name}: path={path}, error={error}")
        }
    }
}

impl Error for CalcArtifactsError {}

pub fn calculate_artifacts(workspace: &mut Workspace, executor: &mut TaskExecutor) -> Result<(), CalcArtifactsError> {
    let mut calc_artifacts_tasks: Vec<Arc<str>> = Vec::new();

    for (_, task) in workspace.tasks.iter() {
        for calc_artifact in task.artifacts.calc.iter() {
            calc_artifacts_tasks.push(calc_artifact.clone());
        }
    }

    // First need to make sure all calculated dependencies in the dependency trees of the calc artifacts tasks are resolved
    resolve_calculated_dependencies_in_subtrees(calc_artifacts_tasks.iter(), workspace, executor)
        .map_err(|e| CalcArtifactsError::DependencyError(e))?;

    // Execute the tasks
    executor.execute_tasks(&workspace, calc_artifacts_tasks.iter())
        .map_err(|e| CalcArtifactsError::ExecutionError(e))?;

    // Swap the calc artifacts for the task outputs of that task
    let mut new_tasks: HashMap<Arc<str>, Arc<Task>> = HashMap::new();
    let executor_cache = executor.cache();
    for (_, task) in workspace.tasks.iter() {
        let mut calc_artifacts: Vec<Arc<str>> = Vec::new();
        for calc_artifact in task.artifacts.calc.iter() {
            let task_outputs = executor_cache.task_outputs.read().unwrap();
            let task_output: HashMap<i64, String> = serde_json::from_value(task_outputs[calc_artifact].clone())
                .map_err(|e| CalcArtifactsError::OutputError { task_name: task.name.clone(), task_output: task_outputs[calc_artifact].clone(), error: e })?;
            for (_i, artifact) in task_output {
                let artifact_path = resolve_path(task.project_path.as_ref(), artifact.as_str())
                    .map_err(|e| CalcArtifactsError::NameResolutionError{ task_name: task.name.clone(), path: artifact.clone(), error: e })?;
                calc_artifacts.push(artifact_path.clone());
            }
        }

        let mut new_task = Task::clone(task);
        new_task.artifacts.files.append(&mut calc_artifacts);
        new_tasks.insert(task.name.clone(), Arc::new(new_task));
    }

    for (task_name, task) in new_tasks {
        workspace.tasks.insert(task_name.clone(), task.clone());
    }

    Ok(())
}