use std::path::Path;

use crate::workspace::{config::{find_nearest_project_dir, get_workspace_config}, load::load_projects, query::{find_project_for_dir, find_tasks_for_dir, get_all_tasks}};


pub struct ListCommandInput<'a> {
    pub cwd: &'a Path,
    pub tasks: Vec<&'a str>
}

pub fn list_command<'a>(input: ListCommandInput<'a>) {
    let config = get_workspace_config(input.cwd).unwrap();
    let projects = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str())).unwrap();

    let workspace = get_all_tasks(projects.values());

    println!("{:?}",  &workspace);

    let project_dir = find_nearest_project_dir(input.cwd, &config.workspace_dir).unwrap();
    let project_for_dir = find_project_for_dir(projects.values(), config.workspace_dir.as_path(), project_dir.as_path()).unwrap();
    let mut tasks_for_dir = find_tasks_for_dir(&workspace, config.workspace_dir.as_path(), project_dir.as_path());

    tasks_for_dir.sort();

    for name in tasks_for_dir {
        let rel_name = name.strip_prefix(project_for_dir.name.as_str())
            .map(|n| n.strip_prefix("/").unwrap_or(n))
            .map(|n| if n.len() > 0 { n } else { "(default)" });

        println!("{}", rel_name.unwrap_or(name));
    }
}