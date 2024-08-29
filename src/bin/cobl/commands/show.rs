use std::{env::set_current_dir, path::PathBuf, sync::Arc};

use cobble::{calc_artifacts::calculate_artifacts, config::{get_workspace_config, TaskOutputCondition, WorkspaceConfigArgs}, dependency::resolve_calculated_dependencies_in_subtrees, execute::execute::TaskExecutor, load::load_projects, task_selection::compute_selected_tasks, workspace::create_workspace};

const TAB: &str = "  ";

pub struct ShowTaskInput {
    pub cwd: PathBuf,
    pub tasks: Vec<String>,
    pub vars: Vec<String>,
    pub num_threads: Option<u8>
}

pub fn show_task_command(input: ShowTaskInput) -> anyhow::Result<()> {
    let ShowTaskInput {
        cwd,
        tasks,
        vars,
        num_threads
    } = input;

    let ws_config_args = WorkspaceConfigArgs {
        vars,
        num_threads: num_threads,
        show_stdout: Some(TaskOutputCondition::Never),
        show_stderr: Some(TaskOutputCondition::Never),
        ..Default::default()
    };

    let config = Arc::new(get_workspace_config(cwd.as_path(), &ws_config_args)?);
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
        return Err(anyhow::anyhow!("No tasks found that match \"{}\"", tasks.join(" ")));
    }

    // Resolve calculated artifacts and dependencies
    let mut executor = TaskExecutor::new(
        config.clone(),
        config.workspace_dir.join(".cobble.db").as_path(),
    )?;

    calculate_artifacts(&mut workspace, &mut executor)?;

    resolve_calculated_dependencies_in_subtrees(
        selected_tasks.iter(),
        &mut workspace,
        &mut executor,
    )?;

    for task_name in selected_tasks.iter() {
        let task = workspace.tasks.get(task_name).unwrap();
        println!("Task: {task_name}");

        println!("{TAB}Artifacts:");
        if task.artifacts.files.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for f in task.artifacts.files.iter() {
                println!("{TAB}{TAB}{f}");
            }
        }
        println!("");

        println!("{TAB}File Dependencies:");
        if task.file_deps.len() == 0 {
            println!("{TAB}{TAB}<none>");
        } else {
            for (_, f) in task.file_deps.iter() {
                print!("{TAB}{TAB}{}", f.path);
                if let Some(provider) = f.provided_by_task.as_ref() {
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
            for (_, t) in task.task_deps.iter() {
                println!("{TAB}{TAB}{t}");
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