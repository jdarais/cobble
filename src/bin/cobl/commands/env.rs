use std::{env::set_current_dir, path::PathBuf, sync::Arc};

use cobble::{
    config::{get_workspace_config, WorkspaceConfigArgs},
    dependency::resolve_calculated_dependencies_in_subtrees,
    execute::execute::TaskExecutor,
    load::load_projects,
    task_selection::compute_selected_tasks,
    workspace::create_workspace,
};

pub struct RunEnvInput {
    pub cwd: PathBuf,
    pub envs: Vec<String>,
    pub args: Vec<String>,
    pub num_threads: Option<u8>,
}

pub fn run_env_command(input: RunEnvInput) -> anyhow::Result<()> {
    let RunEnvInput {
        cwd,
        envs,
        args,
        num_threads,
    } = input;

    let ws_config_args = WorkspaceConfigArgs {
        num_threads: num_threads,
        ..Default::default()
    };
    let config = Arc::new(get_workspace_config(cwd.as_path(), &ws_config_args)?);
    set_current_dir(&config.workspace_dir)
        .expect("found the workspace directory, so we should be able to set that as the cwd");

    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )?;
    let mut workspace = create_workspace(projects.values());

    let selected_tasks = compute_selected_tasks(
        &envs.iter().map(|s| s.as_str()).collect(),
        &workspace,
        cwd.as_path(),
        &config.workspace_dir,
    )?;
    let selected_envs: Vec<Arc<str>> = selected_tasks
        .into_iter()
        .filter(|t| workspace.build_envs.contains_key(t))
        .collect();

    // Resolve calculated dependencies
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;
    resolve_calculated_dependencies_in_subtrees(
        selected_envs.iter(),
        &mut workspace,
        &mut executor,
    )?;

    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;

    let args_arcs: Vec<Arc<str>> = args.into_iter().map(|s| s.into()).collect();

    executor.do_env_actions(&workspace, selected_envs.iter(), &args_arcs)?;

    Ok(())
}
