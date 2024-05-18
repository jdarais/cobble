
use std::path::PathBuf;

use cobble::config::{find_nearest_project_dir, get_workspace_config};
use cobble::workspace::create_workspace;
use cobble::load::load_projects;
use cobble::query::{find_tasks_for_dir, find_tasks_for_query};
use cobble::resolve::project_path_to_project_name;

pub struct ListCommandInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
}

pub fn list_command(input: ListCommandInput) -> anyhow::Result<()> {
    let config = get_workspace_config(input.cwd.as_path(), &Default::default()).unwrap();
    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )?;

    let workspace = create_workspace(projects.values());

    let project_dir = find_nearest_project_dir(input.cwd.as_path(), &config.workspace_dir).unwrap();
    let project_name = project_path_to_project_name(project_dir.as_path()).unwrap();

    let mut tasks = match input.tasks.len() {
        0 => find_tasks_for_dir(
            &workspace,
            config.workspace_dir.as_path(),
            project_dir.as_path(),
        ),
        _ => find_tasks_for_query(
            &workspace,
            project_name.as_str(),
            input.tasks.iter().map(|s| s.as_str()),
        )
        .unwrap(),
    };
    tasks.sort();
    let tasks = tasks;

    for name in tasks {
        let rel_name_opt = name
            .strip_prefix(project_name.as_str())
            .map(|n| n.strip_prefix("/").unwrap_or(n))
            .map(|n| if n.len() > 0 { n } else { "(default)" });

        if let Some(rel_name) = rel_name_opt {
            println!("{}", rel_name);
        }
    }

    Ok(())
}
