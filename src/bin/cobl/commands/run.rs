// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::cmp;
use std::collections::HashMap;
use std::env::set_current_dir;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cobble::calc_artifacts::calculate_artifacts;
use cobble::config::{
    get_workspace_config, parse_cli_vars, TaskOutputCondition, WorkspaceConfig,
    WorkspaceConfigArgs, WorkspaceInit,
};
use cobble::dependency::resolve_calculated_dependencies_in_subtrees;
use cobble::execute::execute::TaskExecutor;
use cobble::load::load_projects;
use cobble::project_def::types::TaskVar;
use cobble::task_selection::compute_selected_tasks;
use cobble::util::process_io::{IOBuffer, ProcessIO, StandardIO};
use cobble::workspace::create_workspace;

pub struct RunCommandInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
    pub vars: Vec<String>,
    pub force_run_tasks: bool,
    pub num_threads: Option<u8>,
    pub show_stdout: Option<TaskOutputCondition>,
    pub show_stderr: Option<TaskOutputCondition>,
}

pub fn run_command(input: RunCommandInput) -> anyhow::Result<()> {
    let RunCommandInput {
        cwd,
        tasks,
        vars,
        force_run_tasks,
        num_threads,
        show_stdout,
        show_stderr,
    } = input;

    let parsed_vars = parse_cli_vars(vars.iter())?;

    run(
        cwd,
        tasks,
        parsed_vars,
        force_run_tasks,
        num_threads,
        show_stdout,
        show_stderr,
        &StandardIO,
    )
}

fn run_init_task<P, IO>(
    cwd: P,
    config: &WorkspaceConfig,
    init_config: &WorkspaceInit,
    pio: &IO,
) -> anyhow::Result<()>
where
    P: AsRef<Path>,
    IO: ProcessIO,
{
    let mut out = pio.out();
    let _ = writeln!(&mut out, "# Running Init Task #");
    let run_res = run(
        cwd.as_ref().join(init_config.workspace_dir.as_path()),
        vec![init_config.task.clone()],
        config.vars.clone(),
        config.force_run_tasks,
        Some(config.num_threads),
        Some(init_config.show_stdout.clone()),
        Some(init_config.show_stderr.clone()),
        pio,
    );
    let _ = writeln!(&mut out, "# Done Running Init Task #");
    run_res
}

pub fn run_init_task_if_defined<P: AsRef<Path>>(
    cwd: P,
    config: &WorkspaceConfig,
) -> anyhow::Result<()> {
    match &config.init {
        Some(init_workspace) => {
            let output_condition =
                cmp::max(&init_workspace.show_stderr, &init_workspace.show_stdout);

            match output_condition {
                TaskOutputCondition::Always => {
                    run_init_task(cwd, config, init_workspace, &StandardIO)
                }
                TaskOutputCondition::OnFail => {
                    let mut io_buffer = IOBuffer::new();
                    let run_res = run_init_task(cwd, config, init_workspace, &io_buffer);

                    if let Err(_) = run_res {
                        let _ = io_buffer.flush();
                    }
                    run_res
                }
                TaskOutputCondition::Never => {
                    let io_buffer = IOBuffer::new();
                    run_init_task(cwd, config, init_workspace, &io_buffer)
                }
            }
        }
        None => Ok(()),
    }
}

pub fn run<IO: ProcessIO>(
    cwd: PathBuf,
    tasks: Vec<String>,
    vars: HashMap<String, TaskVar>,
    force_run_tasks: bool,
    num_threads: Option<u8>,
    show_stdout: Option<TaskOutputCondition>,
    show_stderr: Option<TaskOutputCondition>,
    pio: &IO,
) -> anyhow::Result<()> {
    let mut out = pio.out();

    let ws_config_args = WorkspaceConfigArgs {
        vars,
        force_run_tasks: Some(force_run_tasks),
        num_threads,
        show_stdout,
        show_stderr,
    };
    let config = Arc::new(get_workspace_config(cwd.as_path(), &ws_config_args)?);

    run_init_task_if_defined(&cwd, &config)?;

    set_current_dir(&config.workspace_dir)
        .expect("found the workspace directory, so we should be able to set that as the cwd");

    let projects = load_projects(
        config.workspace_dir.as_path(),
        config.root_projects.iter().map(|s| s.as_str()),
    )?;
    let mut workspace = create_workspace(projects.values());

    let selected_tasks = compute_selected_tasks(
        &tasks.iter().map(|s| s.as_str()).collect(),
        &workspace,
        cwd.as_path(),
        &config.workspace_dir,
    )?;

    if selected_tasks.len() == 0 {
        return Err(anyhow::anyhow!(
            "No tasks found that match \"{}\"",
            tasks.join(" ")
        ));
    }

    // Resolve calculated artifacts and dependencies
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;

    let _ = writeln!(&mut out, "# Computing calculated artifacts #");
    calculate_artifacts(&mut workspace, &mut executor, pio)?;

    let _ = writeln!(&mut out, "# Computing calculated dependencies #");
    resolve_calculated_dependencies_in_subtrees(
        selected_tasks.iter(),
        &mut workspace,
        &mut executor,
        pio,
    )?;

    let _ = writeln!(&mut out, "# Executing tasks #");
    executor.execute_tasks(&workspace, selected_tasks.iter(), pio)?;

    Ok(())
}
