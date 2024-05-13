use std::path::Path;
use std::sync::Arc;

use cobble::config::get_workspace_config;
use cobble::dependency::resolve_calculated_dependencies_in_subtrees;
use cobble::execute::TaskExecutor;
use cobble::workspace::{create_workspace, get_clean_task_name};
use cobble::load::load_projects;
use cobble::task_selection::compute_selected_tasks;

pub struct CleanCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>,
}

pub fn clean_command<'a>(input: CleanCommandInput<'a>) -> anyhow::Result<()> {
    let CleanCommandInput { cwd, tasks } = input;

    let config = Arc::new(get_workspace_config(cwd, &Default::default())?);

    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )?;
    let mut workspace = create_workspace(projects.values());

    let selected_tasks = compute_selected_tasks(&tasks, &workspace, cwd, &config.workspace_dir)?;
    let clean_tasks: Vec<Arc<str>> = selected_tasks
        .iter()
        .map(|s| s.as_ref())
        .map(get_clean_task_name)
        .collect();

    // Resolve calculated dependencies.  Is this needed for clean tasks, given that the only tasks they can rely on are build env tasks?
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;
    resolve_calculated_dependencies_in_subtrees(clean_tasks.iter(), &mut workspace, &mut executor)?;

    // Execute the tasks
    executor.execute_tasks(&workspace, clean_tasks.iter())?;

    Ok(())
}
