use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cobl::workspace::Workspace;
use crate::cobl::resolve::{resolve_name, NameResolutionError};

pub fn find_tasks_for_dir<'a>(
    workspace: &'a Workspace,
    workspace_dir: &Path,
    project_dir: &Path,
) -> Vec<Arc<str>> {
    let full_project_dir = PathBuf::from_iter(workspace_dir.join(project_dir).components());
    workspace
        .tasks
        .iter()
        .filter(|(_k, v)| {
            PathBuf::from_iter(workspace_dir.join(v.dir.as_ref()).components())
                .starts_with(&full_project_dir)
        })
        .map(|(k, _v)| k.clone())
        .collect()
}

pub fn find_tasks_for_query<'i, I>(
    workspace: &Workspace,
    project_name: &str,
    task_queries: I,
) -> Result<Vec<Arc<str>>, NameResolutionError>
where
    I: Iterator<Item = &'i str>,
{
    let mut result: Vec<Arc<str>> = Vec::new();

    for q in task_queries {
        let resolved_q = resolve_name(project_name, &Arc::<str>::from(q))?;
        match workspace.tasks.get_key_value(&resolved_q) {
            Some((k, _)) => {
                result.push(k.clone());
            }
            None => {
                return Err(NameResolutionError::InvalidName(String::from(
                    resolved_q.as_ref(),
                )));
            }
        }
    }

    Ok(result)
}
