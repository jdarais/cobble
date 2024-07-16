use std::{collections::HashMap, sync::Arc};

use crate::{dependency::resolve_calculated_dependencies_in_subtrees, execute::execute::TaskExecutor, workspace::{Task, Workspace}};

pub fn calculate_artifacts(workspace: &mut Workspace, executor: &mut TaskExecutor) -> anyhow::Result<()> {
    let mut calc_artifacts_tasks: Vec<Arc<str>> = Vec::new();

    for (_, task) in workspace.tasks.iter() {
        for calc_artifact in task.artifacts.calc.iter() {
            calc_artifacts_tasks.push(calc_artifact.clone());
        }
    }

    // First need to make sure all calculated dependencies in the dependency trees of the calc artifacts tasks are resolved
    resolve_calculated_dependencies_in_subtrees(calc_artifacts_tasks.iter(), workspace, executor)?;

    // Execute the tasks
    executor.execute_tasks(&workspace, calc_artifacts_tasks.iter())?;

    // Swap the calc artifacts for the task outputs of that task
    let mut new_tasks: HashMap<Arc<str>, Arc<Task>> = HashMap::new();
    let executor_cache = executor.cache();
    for (_, task) in workspace.tasks.iter() {
        let mut calc_artifacts: Vec<Arc<str>> = Vec::new();
        for calc_artifact in task.artifacts.calc.iter() {
            let task_outputs = executor_cache.task_outputs.read().unwrap();
            let mut task_output: Vec<String> = serde_json::from_value(task_outputs[calc_artifact].clone())?;
            for artifact in task_output.drain(..) {
                calc_artifacts.push(artifact.into());
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