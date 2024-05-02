use std::path::Path;

use crate::workspace::graph::Workspace;
use crate::workspace::resolve::{resolve_name, NameResolutionError};

pub fn find_tasks_for_dir<'a>(workspace: &'a Workspace, workspace_dir: &Path, project_dir: &Path) -> Vec<&'a str> {
    let full_project_dir = workspace_dir.join(project_dir);
    workspace.tasks.iter()
        .filter(|(_k, v)| workspace_dir.join(v.dir.as_path()).starts_with(&full_project_dir))
        .map(|(k, _v)| k.as_str())
        .collect()
}

pub fn find_tasks_for_query<'w, 'i, I>(workspace: &'w Workspace, project_name: &str, task_queries: I) -> Result<Vec<&'w str>, NameResolutionError>
    where I: Iterator<Item = &'i str>
{
    let mut result: Vec<&'w str> = Vec::new();

    for q in task_queries {
        let resolved_q = resolve_name(project_name, q)?;
        match workspace.tasks.get_key_value(&resolved_q) {
            Some((k, _)) => { result.push(k.as_str()); },
            None => { return Err(NameResolutionError::InvalidName(resolved_q)); }
        }
    }

    Ok(result)
}

