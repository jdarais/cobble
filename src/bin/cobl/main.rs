mod commands;

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use cobble::config::get_workspace_config;
use cobble::load::load_projects;

use crate::commands::clean::{clean_command, CleanCommandInput};
use crate::commands::list::{list_command, ListCommandInput};
use crate::commands::run::{run_command, RunCommandInput};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    List {
        /// If provided, display only the matched tasks
        tasks: Vec<String>,
    },
    Run {
        /// If not provided, run all tasks in the project
        tasks: Vec<String>,

        /// Set the value for a variable
        #[arg(short, long, value_names(["VAR=VALUE"]), action=clap::ArgAction::Append)]
        var: Vec<String>,

        /// Run tasks even if they are up-to-date
        #[arg(short, long)]
        force_run_tasks: bool,
    },
    Clean {
        /// If not provided, cleans all default tasks, (dependencies are excluded)
        tasks: Vec<String>,
    },
}

fn run_from_dir(path: &Path) -> anyhow::Result<()> {
    let config = get_workspace_config(path, &Default::default()).unwrap();

    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )
    .unwrap();

    println!("Projects:");
    for (name, proj) in projects.iter() {
        println!("\"{}\" = {}", name, proj);
    }

    Ok(())
}

fn main() -> ExitCode {
    let args = Cli::parse();

    let cwd = std::env::current_dir().expect("was run from a directory");

    let result = match args.command {
        Some(cmd) => match cmd {
            Command::List { tasks } => {
                let input = ListCommandInput {
                    cwd: cwd.as_path(),
                    tasks: tasks.iter().map(|s| s.as_str()).collect(),
                };
                list_command(input)
            }
            Command::Run {
                tasks,
                var,
                force_run_tasks,
            } => run_command(RunCommandInput {
                cwd: cwd.as_path(),
                tasks: tasks.iter().map(|s| s.as_str()).collect(),
                vars: var,
                force_run_tasks,
            }),
            Command::Clean { tasks } => clean_command(CleanCommandInput {
                cwd: cwd.as_path(),
                tasks: tasks.iter().map(|s| s.as_str()).collect(),
            }),
        },
        None => run_from_dir(cwd.as_path()),
    };

    match result {
        Ok(_) => ExitCode::from(0),
        Err(e) => {
            println!("{:?}", e);
            ExitCode::from(1)
        }
    }
}
