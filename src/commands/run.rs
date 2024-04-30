use std::path::Path;
use std::process::ExitCode;

use crate::workspace::config::{find_nearest_project_dir, get_workspace_config};
use crate::workspace::execute::TaskExecutor;
use crate::workspace::load::load_projects;
use crate::workspace::query::{create_workspace, find_tasks_for_dir, find_tasks_for_query};
use crate::workspace::resolve::project_path_to_project_name;

pub struct RunCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>
}

pub fn run_command<'a>(input: RunCommandInput<'a>) -> ExitCode {
    let config = get_workspace_config(input.cwd).unwrap();
    let projects = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str())).unwrap();

    let workspace = create_workspace(projects.values());

    let project_dir = find_nearest_project_dir(input.cwd, &config.workspace_dir).unwrap();
    let project_name = project_path_to_project_name(project_dir.as_path()).unwrap();

    let mut tasks = match input.tasks.len() {
        0 => find_tasks_for_dir(&workspace, config.workspace_dir.as_path(), project_dir.as_path()),
        _ => find_tasks_for_query(&workspace, project_name.as_str(), input.tasks.iter().copied()).unwrap()
    };
    tasks.sort();
    let tasks = tasks;

    let mut executor = TaskExecutor::new(config.workspace_dir.as_path(), config.workspace_dir.join(".cobble.db").as_path());
    let result = executor.execute_tasks(&workspace, tasks.iter().copied());

    match result {
        Ok(_) => ExitCode::from(0),
        Err(e) => {
            println!("{}", e);
            return ExitCode::from(1)
        }
    }
}