use std::env::set_current_dir;
use std::path::PathBuf;
use std::sync::Arc;

use cobble::config::get_workspace_config;
use cobble::dependency::resolve_calculated_dependencies_in_subtrees;
use cobble::execute::execute::TaskExecutor;
use cobble::load::load_projects;
use cobble::task_selection::compute_selected_tasks;
use cobble::workspace::create_workspace;

pub struct CleanCommandInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
}

pub fn clean_command<'a>(input: CleanCommandInput) -> anyhow::Result<()> {
    let CleanCommandInput { cwd, tasks } = input;

    let config = Arc::new(get_workspace_config(cwd.as_path(), &Default::default())?);
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

    // Resolve calculated dependencies.  Is this needed for clean tasks, given that the only tasks they can rely on are build env tasks?
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;
    resolve_calculated_dependencies_in_subtrees(selected_tasks.iter(), &mut workspace, &mut executor)?;

    // Execute the tasks
    executor.clean_tasks(&workspace, selected_tasks.iter())?;

    Ok(())
}
