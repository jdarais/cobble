// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::env::set_current_dir;
use std::path::PathBuf;

use cobble::config::{find_nearest_project_dir, get_workspace_config};
use cobble::load::load_projects;
use cobble::query::{find_tasks_for_dir, find_tasks_for_query};
use cobble::resolve::project_path_to_project_name;
use cobble::workspace::create_workspace;

use crate::commands::run::run_init_task_if_defined;

const MAX_DESCRIPTION_LENGTH: usize = 80;

pub struct ListCommandInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
    pub all: bool,
}

pub fn list_command(input: ListCommandInput) -> anyhow::Result<()> {
    let config = get_workspace_config(input.cwd.as_path(), &Default::default()).unwrap();

    run_init_task_if_defined(&input.cwd, &config)?;

    set_current_dir(&config.workspace_dir)
        .expect("found the workspace directory, so we should be able to set that as the cwd");

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
            input.all
        ),
        _ => find_tasks_for_query(
            &workspace,
            project_name.as_str(),
            input.tasks.iter().map(|s| s.as_str()),
            input.all,
        )
        .unwrap(),
    };
    tasks.sort();
    let tasks = tasks;

    let mut rows: Vec<(&str, &str)> = Vec::new();

    for name in tasks.iter() {
        let task = workspace.tasks.get(name).expect("Task name found in workspace should exist in the workspace");
        let maybe_rel_name = name
            .strip_prefix(project_name.as_str())
            .map(|n| n.strip_prefix("/").unwrap_or(n))
            .map(|n| if n.len() > 0 { n } else { "(default)" })
            .unwrap_or(name.as_ref());

        rows.push((maybe_rel_name, task.description.as_ref()))
    }
    
    let max_task_name_width = rows.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    let task_name_column_width = (max_task_name_width + 8) + (max_task_name_width % 4);

    for (name, desc) in rows {
        let truncated = desc.len() > MAX_DESCRIPTION_LENGTH;
        let desc_truncated = if truncated { &desc[..MAX_DESCRIPTION_LENGTH-3] } else { desc };
        let maybe_ellipsis = if truncated { "..." } else { "" };
        println!("{:<task_name_column_width$}{}{}", name, desc_truncated, maybe_ellipsis);
    }
    Ok(())
}
