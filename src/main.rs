extern crate clap;

mod commands;
mod datamodel;
mod lua;
mod workspace;

use std::path::Path;

use clap::{Parser, Subcommand};

use workspace::load::{
    extract_project_defs,
    init_lua_for_project_config,
    process_project_file,
};

use workspace::config::get_workspace_config;

use lua::lua_env::create_lua_env;

use crate::datamodel::Project;
use crate::workspace::load::load_projects;
use crate::workspace::resolve::{resolve_names_in_project, NameResolutionError};

#[derive(Parser)]
struct Cli {

    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand)]
enum Command {
    List {
        /// If provided, display only the matched tasks
        tasks: Vec<String>
    },
    Run {

    }
}


fn run_from_dir(path: &Path) {
    let config = get_workspace_config(path).unwrap();

    let projects = load_projects(config.workspace_dir.as_path(), config.root_projects.iter().map(|s| s.as_str())).unwrap();

    println!("Projects:");
    for (name, proj) in projects.iter() {
        println!("\"{}\" = {}", name, proj);
    }
}

fn main() {
    let args = Cli::parse();
    
    let cwd = std::env::current_dir().expect("was run from a directory");

    run_from_dir(cwd.as_path())
}
