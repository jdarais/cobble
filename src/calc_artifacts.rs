// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{borrow::Cow, collections::BTreeMap, error::Error, fmt, path::Path, sync::Arc};

use crate::{
    dependency::{resolve_calculated_dependencies_in_subtrees, ExecutionGraphError},
    execute::execute::{TaskExecutionError, TaskExecutor},
    lua::s11n::SerLuaValueBlock,
    project_def::{
        types::{extend_string_or_int_table, StringOrInt},
        Artifacts,
    },
    resolve::{resolve_names_in_artifacts, NameResolutionError},
    util::process_io::ProcessIO,
    workspace::{Task, Workspace},
};

#[derive(Debug)]
pub enum CalcArtifactsError {
    DependencyError {
        task_name: Arc<str>,
        error: ExecutionGraphError,
    },
    ExecutionError(TaskExecutionError),
    OutputError {
        for_task_name: Arc<str>,
        task_name: Arc<str>,
        task_output: SerLuaValueBlock,
        error: String,
    },
    NameResolutionError {
        task_name: Arc<str>,
        error: NameResolutionError,
    },
}

impl fmt::Display for CalcArtifactsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CalcArtifactsError::*;
        match self {
            DependencyError{ task_name, error } => write!(f, "Error resolving dependencies in calc artifacts tasks for {task_name}: {error}"),
            ExecutionError(e) => write!(f, "Error while executing artifacts task or one of its dependencies: {e}"),
            OutputError{ for_task_name, task_name, task_output, error } => write!(f, "Error in output of calc artifacts task {task_name} for {for_task_name}: output={task_output}, error={error}"),
            NameResolutionError{ task_name, error } => write!(f, "Error resolving calc artifact path returned by task {task_name}: error={error}")
        }
    }
}

impl Error for CalcArtifactsError {}

fn combine_artifacts(lhs: &Artifacts, rhs: &Artifacts) -> Artifacts {
    let mut files: BTreeMap<StringOrInt, Arc<str>> = lhs.files.clone();
    extend_string_or_int_table(
        &mut files,
        rhs.files.iter().map(|(k, v)| (k.clone(), v.clone())),
    );

    Artifacts {
        files,
        calc: lhs.calc.iter().chain(rhs.calc.iter()).cloned().collect(),
    }
}

pub fn calculate_artifacts<IO: ProcessIO>(
    ws_dir: &Path,
    workspace: &mut Workspace,
    executor: &mut TaskExecutor,
    pio: &IO,
) -> Result<(), CalcArtifactsError> {
    let mut calc_artifacts_tasks: Vec<Arc<str>> = Vec::new();

    for (task_name, task) in workspace.tasks.clone().iter() {
        for calc_artifact in task.artifacts.calc.iter() {
            calc_artifacts_tasks.push(calc_artifact.clone());
        }

        // First need to make sure all calculated dependencies in the dependency trees of the calc artifacts tasks are resolved
        resolve_calculated_dependencies_in_subtrees(
            ws_dir,
            task.artifacts.calc.iter(),
            &mut *workspace,
            executor,
            pio,
        )
        .map_err(|e| CalcArtifactsError::DependencyError {
            task_name: task_name.clone(),
            error: e,
        })?;
    }

    // Execute the tasks
    executor
        .execute_tasks(&workspace, calc_artifacts_tasks.iter(), pio)
        .map_err(|e| CalcArtifactsError::ExecutionError(e))?;

    // Swap the calc artifacts for the task outputs of that task
    let mut updated_tasks: BTreeMap<Arc<str>, Arc<Task>> = BTreeMap::new();
    let executor_cache = executor.cache();
    for (_, task) in workspace.tasks.iter() {
        let mut artifacts: Cow<Artifacts> = Cow::Borrowed(&task.artifacts);
        for calc_artifact in task.artifacts.calc.iter() {
            let task_outputs = executor_cache.task_outputs.read().unwrap();
            let task_output_record = Artifacts::try_from(&task_outputs[calc_artifact].task_output)
                .map_err(|e| CalcArtifactsError::OutputError {
                    for_task_name: task.name.clone(),
                    task_name: calc_artifact.clone(),
                    task_output: task_outputs[calc_artifact].task_output.clone(),
                    error: e,
                })?;
            let mut task_output_artifacts: Artifacts = task_output_record.into();
            resolve_names_in_artifacts(
                ws_dir,
                &task.project_name,
                &task.project_path,
                &mut task_output_artifacts,
            )
            .map_err(|e| CalcArtifactsError::NameResolutionError {
                task_name: task.name.clone(),
                error: e,
            })?;

            artifacts = Cow::Owned(combine_artifacts(
                artifacts.as_ref(),
                &task_output_artifacts,
            ));
        }

        let mut updated_task = Task::clone(task);
        updated_task.artifacts = artifacts.as_ref().clone();
        updated_tasks.insert(task.name.clone(), Arc::new(updated_task));
    }

    for (task_name, task) in updated_tasks {
        workspace.tasks.insert(task_name.clone(), task.clone());

        // Need to update file providers map as well
        // TODO: Make it an error if a file is already provided by another task (need to do this in
        // compute_file_providers function as well.)
        for (_file_alias, file_path) in task.artifacts.files.iter() {
            workspace.file_providers.insert(file_path.clone(), task_name.clone());
        }
    }

    Ok(())
}
