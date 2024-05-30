mod commands;

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use cobble::config::{get_workspace_config, DEFAULT_NUM_THREADS};
use cobble::load::load_projects;

use crate::commands::clean::{clean_command, CleanCommandInput};
use crate::commands::env::{run_env_command, RunEnvInput};
use crate::commands::list::{list_command, ListCommandInput};
use crate::commands::run::{run_command, RunCommandInput};
use crate::commands::tool::{check_tool_command, CheckToolInput};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<CoblCommand>,

    #[arg(help = format!("The number of threads to use for running tasks. (Default: {})", DEFAULT_NUM_THREADS))]
    #[arg(short, long, global(true), value_name("N"))]
    num_threads: Option<u8>,

    /// Set the value for a variable
    #[arg(short, long, value_names(["VAR=VALUE"]), global(true), action=clap::ArgAction::Append)]
    var: Vec<String>,

    /// Display the version of this application and exit
    #[arg(long)]
    version: bool
}

#[derive(Subcommand)]
enum CoblCommand {
    /// List available tasks
    List {
        /// If provided, display only the matched tasks
        tasks: Vec<String>,
    },
    /// Run tasks
    Run {
        /// If not provided, run all tasks in the project
        tasks: Vec<String>,

        /// Run tasks even if they are up-to-date
        #[arg(short, long)]
        force: bool,
    },
    /// Clean tasks
    Clean {
        /// If not provided, cleans all default tasks, (dependencies are excluded)
        tasks: Vec<String>,
    },
    /// Interact with tools defined in the workspace
    Tool {
        #[command(subcommand)]
        tool_cmd: ToolCommand,
    },
    /// Interact with build environments defined in the workspace
    Env {
        #[command(subcommand)]
        env_cmd: EnvCommand
    }
}

#[derive(Subcommand)]
enum ToolCommand {
    Check {
        /// Tool names
        names: Vec<String>,
    },
}

#[derive(Subcommand)]
enum EnvCommand {
    Run {
        envs: Vec<String>,

        #[arg(last(true))]
        args: Vec<String>
    }
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

    if args.version {
        println!("{}", VERSION);
        return ExitCode::from(0);
    }

    let cwd = std::env::current_dir().expect("was run from a directory");

    let result = match args.command {
        Some(cmd) => match cmd {
            CoblCommand::List { tasks } => list_command(ListCommandInput {
                cwd: cwd,
                tasks: tasks,
            }),
            CoblCommand::Run {
                tasks,
                force,
            } => run_command(RunCommandInput {
                cwd,
                tasks,
                vars: args.var,
                force_run_tasks: force,
                num_threads: args.num_threads
            }),
            CoblCommand::Clean { tasks } => clean_command(CleanCommandInput { cwd, tasks, num_threads: args.num_threads }),
            CoblCommand::Tool { tool_cmd } => match tool_cmd {
                ToolCommand::Check { names } => {
                    check_tool_command(CheckToolInput { cwd, tools: names, num_threads: args.num_threads })
                }
            },
            CoblCommand::Env { env_cmd } => match env_cmd {
                EnvCommand::Run { envs, args: env_args } => run_env_command(RunEnvInput { cwd, envs, args: env_args, num_threads: args.num_threads })
            }
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
