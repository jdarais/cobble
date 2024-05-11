use std::{path::Path, sync::Arc};

use crate::workspace::{config::find_nearest_project_dir, graph::Workspace, query::{find_tasks_for_dir, find_tasks_for_query}, resolve::project_path_to_project_name};



pub fn compute_selected_tasks(task_queries: &Vec<&str>, workspace: &Workspace, cwd: &Path, ws_dir: &Path) -> anyhow::Result<Vec<Arc<str>>> {
    let project_dir = find_nearest_project_dir(cwd, ws_dir)?;
    let project_name = project_path_to_project_name(project_dir.as_path())?;

    let selected_tasks = match task_queries.len() {
        0 => find_tasks_for_dir(&workspace, ws_dir, project_dir.as_path()),
        _ => find_tasks_for_query(&workspace, project_name.as_str(), task_queries.iter().copied())?
    };

    Ok(selected_tasks)
}
