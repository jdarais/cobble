use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use crate::workspace::graph::create_workspace;
use crate::workspace::config::{add_cli_vars_to_workspace_config, find_nearest_project_dir, get_workspace_config};
use crate::workspace::dependency::{compute_file_providers, resolve_calculated_dependencies_in_subtree};
use crate::workspace::execute::TaskExecutor;
use crate::workspace::load::load_projects;
use crate::workspace::query::{find_tasks_for_dir, find_tasks_for_query};
use crate::workspace::resolve::project_path_to_project_name;

pub struct RunCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>,
    pub vars: Vec<String>,
    pub force_run_tasks: bool
}

pub fn run_command<'a>(input: RunCommandInput<'a>) -> ExitCode {
    let mut config = get_workspace_config(input.cwd).unwrap();
    add_cli_vars_to_workspace_config(input.vars.iter().map(String::as_ref), &mut config).unwrap();
    config.force_run_tasks = input.force_run_tasks;
    let config = Arc::new(config);

    let projects_res = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str()));
    let projects = match projects_res {
        Ok(p) => p,
        Err(e) => {
            println!("Encountered an error while loading projects:\n{}", e);
            return ExitCode::from(1);
        }
    };

    let file_providers = compute_file_providers(projects.values());
    let mut workspace = create_workspace(projects.values(), &file_providers);

    let project_dir = find_nearest_project_dir(input.cwd, &config.workspace_dir).unwrap();
    let project_name = project_path_to_project_name(project_dir.as_path()).unwrap();

    let mut tasks = match input.tasks.len() {
        0 => find_tasks_for_dir(&workspace, config.workspace_dir.as_path(), project_dir.as_path()),
        _ => find_tasks_for_query(&workspace, project_name.as_str(), input.tasks.iter().copied()).unwrap()
    };
    tasks.sort();
    let tasks = tasks;

    // Resolve calculated dependencies
    let mut executor = TaskExecutor::new(config.clone(), config.workspace_dir.join(".cobble.db").as_path());
    for task in tasks.iter() {
        resolve_calculated_dependencies_in_subtree(&task, &file_providers, &mut workspace, &mut executor).unwrap();

    }

    let result = executor.execute_tasks(&workspace, tasks.iter());

    match result {
        Ok(_) => ExitCode::from(0),
        Err(e) => {
            println!("{}", e);
            return ExitCode::from(1)
        }
    }
}