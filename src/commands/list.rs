use std::path::PathBuf;

use crate::workspace::{config::get_workspace_config, load::load_projects};


pub struct ListCommandInput<'a> {
    cwd: PathBuf,
    tasks: Vec<&'a str>
}

pub fn list_command<'a>(input: ListCommandInput<'a>) {
    let config = get_workspace_config(input.cwd.as_path()).unwrap();
    let projects = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str())).unwrap();
}