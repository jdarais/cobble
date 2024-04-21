use std::path::PathBuf;

use crate::workspace::config::get_workspace_config;


pub struct ListCommandInput<'a> {
    cwd: PathBuf,
    tasks: Vec<&'a str>
}

pub fn list_command<'a>(input: ListCommandInput<'a>) {
    let config = get_workspace_config(input.cwd.as_path()).unwrap();
}