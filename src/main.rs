// For now, allow dead code, since a lot of large areas of the program are still under construction
#![allow(dead_code)]

extern crate clap;

mod commands;
mod datamodel;
mod lua;
mod workspace;

use std::path::Path;

use clap::{Parser, Subcommand};

use workspace::config::get_workspace_config;

use crate::{commands::{list::{list_command, ListCommandInput}, run::{run_command, RunCommandInput}}, workspace::load::load_projects};

#[derive(Parser)]
struct Cli {

    #[command(subcommand)]
    command: Option<Command>
}

#[derive(Subcommand)]
enum Command {
    List {
        /// If provided, display only the matched tasks
        tasks: Vec<String>
    },
    Run {
        /// If not provided, run all tasks in the project
        tasks: Vec<String>
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

    match &args.command {
        Some(cmd) =>     match cmd {
            Command::List{tasks} => {
                let input = ListCommandInput {
                    cwd: cwd.as_path(),
                    tasks: tasks.iter().map(|s| s.as_str()).collect()
                };
                list_command(input);
            },
            Command::Run{tasks} => {
                run_command(RunCommandInput {
                    cwd: cwd.as_path(),
                    tasks: tasks.iter().map(|s| s.as_str()).collect()
                });
            }
        },
        None => {
            run_from_dir(cwd.as_path());
        }
    }
}
