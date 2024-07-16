use std::env::set_current_dir;
use std::path::PathBuf;
use std::sync::Arc;

use cobble::calc_artifacts::calculate_artifacts;
use cobble::config::{get_workspace_config, TaskOutputCondition, WorkspaceConfigArgs};
use cobble::dependency::resolve_calculated_dependencies_in_subtrees;
use cobble::execute::execute::TaskExecutor;
use cobble::load::load_projects;
use cobble::task_selection::compute_selected_tasks;
use cobble::workspace::create_workspace;

pub struct RunCommandInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
    pub vars: Vec<String>,
    pub force_run_tasks: bool,
    pub num_threads: Option<u8>,
    pub show_stdout: Option<TaskOutputCondition>,
    pub show_stderr: Option<TaskOutputCondition>
}

pub fn run_command(input: RunCommandInput) -> anyhow::Result<()> {
    let RunCommandInput {
        cwd,
        tasks,
        vars,
        force_run_tasks,
        num_threads,
        show_stdout,
        show_stderr
    } = input;

    let ws_config_args = WorkspaceConfigArgs {
        vars,
        force_run_tasks: Some(force_run_tasks),
        num_threads: num_threads,
        show_stdout,
        show_stderr
    };
    let config = Arc::new(get_workspace_config(cwd.as_path(), &ws_config_args)?);
    set_current_dir(&config.workspace_dir)
        .expect("found the workspace directory, so we should be able to set that as the cwd");

    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )?;
    let mut workspace = create_workspace(projects.values());

    let selected_tasks = compute_selected_tasks(
        &tasks.iter().map(|s| s.as_str()).collect(),
        &workspace,
        cwd.as_path(),
        &config.workspace_dir,
    )?;

    // Resolve calculated dependencies
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;

    calculate_artifacts(&mut workspace, &mut executor)?;

    resolve_calculated_dependencies_in_subtrees(
        selected_tasks.iter(),
        &mut workspace,
        &mut executor,
    )?;

    // Execute the tasks
    executor.execute_tasks(&workspace, selected_tasks.iter())?;

    Ok(())
}
