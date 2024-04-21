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
    let workspace_config = get_workspace_config(path).unwrap();

    let project_def_lua = create_lua_env(workspace_config.workspace_dir.as_path()).unwrap();

    init_lua_for_project_config(&project_def_lua, workspace_config.workspace_dir.as_path()).unwrap();

    for project_dir in &workspace_config.root_projects {
        process_project_file(&project_def_lua, project_dir.as_str(), workspace_config.workspace_dir.as_path()).unwrap();
    }

    let projects = extract_project_defs(&project_def_lua).unwrap();

    println!("Projects:");
    for (name, proj) in projects.iter() {
        println!("\"{}\" = {}", name, proj);
    }

    let package_path: String = project_def_lua.load("package.path").eval().unwrap();
    println!("package.path={}", package_path.as_str());
}

fn main() {
    let args = Cli::parse();
    
    let cwd = std::env::current_dir().expect("was run from a directory");

    run_from_dir(cwd.as_path())
}
