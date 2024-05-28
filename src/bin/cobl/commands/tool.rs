use std::{env::set_current_dir, path::PathBuf, sync::Arc};

use cobble::{config::{get_workspace_config, WorkspaceConfigArgs}, execute::execute::TaskExecutor, load::load_projects, workspace::create_workspace};




pub struct CheckToolInput {
    pub cwd: PathBuf,
    pub tools: Vec<String>,
    pub num_threads: Option<u8>
}


pub fn check_tool_command(input: CheckToolInput) -> anyhow::Result<()> {
    let CheckToolInput { cwd, tools, num_threads } = input;

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
    let workspace = create_workspace(projects.values());

    // TODO: Tool name resolution based on wildcards
    let selected_tools: Vec<Arc<str>> = tools.into_iter().map(|s| s.into()).collect();

    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path()
    )?;

    executor.check_tools(&workspace, selected_tools.iter())?;

    Ok(())
}
