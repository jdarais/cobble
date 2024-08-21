// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::path::{Path, PathBuf};
use std::sync::Arc;

use wildmatch::WildMatch;

use crate::resolve::NameResolutionError;
use crate::workspace::Workspace;

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

    let query_patterns: Vec<WildMatch> = task_queries.map(WildMatch::new).collect();
    let mut project_prefix = project_name.to_owned();
    project_prefix.push('/');

    for task_name in workspace.tasks.keys() {
        for query_pattern in &query_patterns {
            if query_pattern.matches(task_name.as_ref())
                || (task_name.starts_with(project_prefix.as_str())
                    && query_pattern.matches(&task_name[project_prefix.len()..]))
            {
                result.push(task_name.clone());
                break;
            }
        }
    }

    Ok(result)
}

pub fn find_envs_for_query<'i, I>(
    workspace: &Workspace,
    project_name: &str,
    env_queries: I,
) -> Result<Vec<Arc<str>>, NameResolutionError>
where
    I: Iterator<Item = &'i str>,
{
    let mut result: Vec<Arc<str>> = Vec::new();

    let query_patterns: Vec<WildMatch> = env_queries.map(WildMatch::new).collect();
    let mut project_prefix = project_name.to_owned();
    project_prefix.push('/');

    for env_name in workspace.build_envs.keys() {
        for query_pattern in &query_patterns {
            if query_pattern.matches(env_name.as_ref())
                || (env_name.starts_with(project_prefix.as_str())
                    && query_pattern.matches(&env_name[project_prefix.len()..]))
            {
                result.push(env_name.clone());
                break;
            }
        }
    }

    Ok(result)
}
