// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::collections::HashSet;
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
    let mut direct_name_matches: HashSet<Arc<str>> = HashSet::new();

    let mut query_patterns: Vec<WildMatch> = Vec::new(); // task_queries.map(WildMatch::new).collect();
    let mut project_prefix = project_name.to_owned();
    project_prefix.push('/');

    // Find all direct name matches, and defer patterns for a separate search loop
    for query in task_queries {
        if query.contains(|ch| ch == '*' || ch == '?') {
            query_patterns.push(WildMatch::new(query));
        } else {
            let full_task_name: Arc<str> = if query.starts_with("/") {
                String::from(query).into()
            } else {
                let mut s = project_prefix.clone();
                s.push_str(query);
                s.into()
            };
            
            if workspace.tasks.contains_key(full_task_name.as_ref()) {
                direct_name_matches.insert(full_task_name.clone());
                result.push(full_task_name.clone());
            } else {
                return Err(NameResolutionError::InvalidName(String::from(query)));
            }
        }
    }

    // Find all pattern matches
    for task_name in workspace.tasks.keys() {
        if direct_name_matches.contains(task_name) {
            continue;
        }

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::workspace::Task;

    use super::*;

    fn create_minimal_workspace() -> Workspace {
        let mut tasks: HashMap<Arc<str>, Arc<Task>> = HashMap::new();
        tasks.insert(String::from("/project1/task1").into(), Arc::new(Default::default()));
        tasks.insert(String::from("/project1/task2").into(), Arc::new(Default::default()));
        tasks.insert(String::from("/project2/task1").into(), Arc::new(Default::default()));

        Workspace {
            tasks: tasks,
            build_envs: HashMap::new(),
            tools: HashMap::new(),
            file_providers: HashMap::new()
        }
    }

    #[test]
    fn test_match_full_task_name() {
        let ws = create_minimal_workspace();

        let matches = find_tasks_for_query(
            &ws,
            "/project1",
            vec!["/project2/task1"].into_iter()
        ).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_ref(), "/project2/task1");
    }

    #[test]
    fn test_match_relative_task_name() {
        let ws = create_minimal_workspace();

        let matches = find_tasks_for_query(
            &ws,
            "/project1",
            vec!["task1"].into_iter()
        ).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_ref(), "/project1/task1");
    }

    #[test]
    fn test_direct_query_no_match_returns_error() {
        let ws = create_minimal_workspace();

        find_tasks_for_query(
            &ws,
            "/project1",
            vec!["not_a_task"].into_iter()
        ).expect_err("Expected 'not_a_task' query to return an error");
    }

    #[test]
    fn test_wildcard_with_no_matches_returns_empty_list() {
        let ws = create_minimal_workspace();

        let matches = find_tasks_for_query(
            &ws,
            "/project1",
            vec!["*/not_a_task"].into_iter()
        ).unwrap();

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_wildcard_with_multiple_matches() {
        let ws = create_minimal_workspace();

        let matches = find_tasks_for_query(
            &ws,
            "/project1",
            vec!["*/task1"].into_iter()
        ).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&String::from("/project1/task1").into()));
        assert!(matches.contains(&String::from("/project2/task1").into()));
    }

    #[test]
    fn test_relative_wildcard() {
        let ws = create_minimal_workspace();

        let matches = find_tasks_for_query(
            &ws,
            "/project1",
            vec!["task?"].into_iter()
        ).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&String::from("/project1/task1").into()));
        assert!(matches.contains(&String::from("/project1/task2").into()));
    }
}
