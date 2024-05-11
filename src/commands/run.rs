use std::path::Path;
use std::sync::Arc;

use crate::workspace::graph::create_workspace;
use crate::workspace::config::{get_workspace_config, WorkspaceConfigArgs};
use crate::workspace::dependency::resolve_calculated_dependencies_in_subtrees;
use crate::workspace::execute::TaskExecutor;
use crate::workspace::load::load_projects;
use crate::workspace::task_selection::compute_selected_tasks;

pub struct RunCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>,
    pub vars: Vec<String>,
    pub force_run_tasks: bool
}

pub fn run_command<'a>(input: RunCommandInput<'a>) -> anyhow::Result<()> {
    let RunCommandInput { cwd, tasks, vars, force_run_tasks } = input;
    let ws_config_args = WorkspaceConfigArgs {
        vars,
        force_run_tasks: Some(force_run_tasks)
    };
    let config = Arc::new(get_workspace_config(cwd, &ws_config_args)?);

    let projects = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str()))?;
    let mut workspace = create_workspace(projects.values());

    let selected_tasks = compute_selected_tasks(&tasks, &workspace, cwd, &config.workspace_dir)?;

    // Resolve calculated dependencies
    let mut executor = TaskExecutor::new(config.clone(), config.workspace_dir.join(".cobble.db").as_path());
    resolve_calculated_dependencies_in_subtrees(selected_tasks.iter(), &mut workspace, &mut executor)?;

    // Execute the tasks
    executor.execute_tasks(&workspace, selected_tasks.iter())?;

    Ok(())
}