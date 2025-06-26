use std::{env::set_current_dir, path::PathBuf, sync::Arc};

use cobble::{
    calc_artifacts::calculate_artifacts,
    config::{get_workspace_config, parse_cli_vars, TaskOutputCondition, WorkspaceConfigArgs},
    dependency::resolve_calculated_dependencies_in_subtrees,
    execute::execute::TaskExecutor,
    load::load_projects,
    task_selection::compute_selected_tasks,
    util::process_io::StandardIO,
    workspace::create_workspace,
};

use crate::commands::run::run_init_task_if_defined;

const TAB: &str = "  ";

pub struct ShowTaskInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
    pub vars: Vec<String>,
    pub num_threads: Option<u8>,
}

pub fn show_task_command(input: ShowTaskInput) -> anyhow::Result<()> {
    let ShowTaskInput {
        cwd,
        tasks,
        vars,
        num_threads,
    } = input;

    let parsed_vars = parse_cli_vars(vars.iter())?;

    let ws_config_args = WorkspaceConfigArgs {
        vars: parsed_vars,
        num_threads: num_threads,
        show_stdout: Some(TaskOutputCondition::Never),
        show_stderr: Some(TaskOutputCondition::Never),
        ..Default::default()
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

    println!("# Computing calculated artifacts #");
    calculate_artifacts(
        &config.workspace_dir,
        &mut workspace,
        &mut executor,
        &StandardIO,
    )?;

    println!("# Computing calculated dependencies #");
    resolve_calculated_dependencies_in_subtrees(
        &config.workspace_dir,
        selected_tasks.iter(),
        &mut workspace,
        &mut executor,
        &StandardIO,
    )?;

    for task_name in selected_tasks.iter() {
        let task = workspace.tasks.get(task_name).unwrap();
        println!("Task: {task_name}");

        println!("{TAB}Artifacts:");
        if task.artifacts.files.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (f_alias, f_path) in task.artifacts.files.iter() {
                println!("{TAB}{TAB}[{f_alias}] = {f_path}");
            }
        }
        println!("");

        println!("{TAB}File Dependencies:");
        if task.file_deps.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (f_alias, f_dep) in task.file_deps.iter() {
                print!("{TAB}{TAB}[{f_alias}] = {}", f_dep.path);
                if let Some(provider) = f_dep.provided_by_task.as_ref() {
                    print!("  (*provided by {})", provider);
                }
                println!("");
            }
        }
        println!("");

        println!("{TAB}Task Dependencies:");
        if task.task_deps.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (t_alias, t_name) in task.task_deps.iter() {
                println!("{TAB}{TAB}[{t_alias}] = {t_name}");
            }
        }
        println!("");

        println!("{TAB}Envs:");
        if task.build_envs.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (_, env) in task.build_envs.iter() {
                println!("{TAB}{TAB}{env}");
            }
        }
        println!("");

        println!("{TAB}Tools:");
        if task.tools.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (_, tool) in task.tools.iter() {
                println!("{TAB}{TAB}{tool}");
            }
        }
        println!("");
    }

    Ok(())
}
