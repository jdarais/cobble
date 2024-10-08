// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use crate::config::WorkspaceConfig;
use crate::execute::action::init_lua_for_task_executor;
use crate::execute::clean_task_job::execute_clean_job;
use crate::execute::env_action_job::execute_env_action_job;
use crate::execute::execute::{ExecutorJob, TaskExecutorCache, TaskJobMessage};
use crate::execute::task_job::execute_task_job;
use crate::execute::tool_check_job::execute_tool_check_job;
use crate::lua::lua_env::create_lua_env;

pub struct TaskExecutorWorkerArgs {
    pub workspace_config: Arc<WorkspaceConfig>,
    pub db_env: Arc<lmdb::Environment>,
    pub db: lmdb::Database,
    pub task_queue: Arc<(Mutex<Option<VecDeque<ExecutorJob>>>, Condvar)>,
    pub task_result_sender: Sender<TaskJobMessage>,
    pub cache: Arc<TaskExecutorCache>,
}

fn poll_next_task(
    task_queue: &(Mutex<Option<VecDeque<ExecutorJob>>>, Condvar),
) -> Option<ExecutorJob> {
    let (task_queue_mutex, task_queue_cvar) = task_queue;
    let mut task_queue_locked = task_queue_mutex.lock().unwrap();

    loop {
        let task_available = match &*task_queue_locked {
            Some(queue) => !queue.is_empty(),
            None => {
                return None;
            }
        };

        if task_available {
            break;
        }

        task_queue_locked = task_queue_cvar.wait(task_queue_locked).unwrap();
    }

    let task_queue = task_queue_locked.as_mut()
        .expect("Task queue should still exist since we are still holding the mutex after validating it exists.");

    let next_task = task_queue.pop_front()
        .expect("Task queue should still have an item since we are still holding the mutex after validating an item is present");

    Some(next_task)
}

pub fn run_task_executor_worker(args: TaskExecutorWorkerArgs) {
    let lua = create_lua_env(args.workspace_config.workspace_dir.as_path())
        .expect("Lua environment creation should always succeed");
    init_lua_for_task_executor(&lua)
        .expect("Initializing lua environment for a task executor should always succeed");

    loop {
        let next_task_opt = poll_next_task(&args.task_queue);

        let next_task = match next_task_opt {
            Some(next_task) => next_task,
            None => {
                return;
            }
        };

        let stdin_ready = Arc::new((Mutex::new(false), Condvar::new()));

        match next_task {
            ExecutorJob::Task(task) => {
                let show_stdout = task
                    .task
                    .show_stdout
                    .as_ref()
                    .map(|c| c.clone())
                    .unwrap_or_else(|| args.workspace_config.show_stdout.clone());
                let show_stderr = task
                    .task
                    .show_stderr
                    .as_ref()
                    .map(|c| c.clone())
                    .unwrap_or_else(|| args.workspace_config.show_stderr.clone());
                args.task_result_sender
                    .send(TaskJobMessage::Started {
                        task: task.task_name.clone(),
                        stdin_ready: stdin_ready.clone(),
                        show_stdout,
                        show_stderr,
                    })
                    .unwrap();
                execute_task_job(
                    &args.workspace_config,
                    &lua,
                    &args.db_env,
                    &args.db,
                    &task,
                    &args.task_result_sender,
                    &stdin_ready,
                    args.cache.clone(),
                );
            }
            ExecutorJob::Clean(clean) => {
                let show_stdout = clean
                    .task
                    .show_stdout
                    .as_ref()
                    .map(|c| c.clone())
                    .unwrap_or_else(|| args.workspace_config.show_stdout.clone());
                let show_stderr = clean
                    .task
                    .show_stderr
                    .as_ref()
                    .map(|c| c.clone())
                    .unwrap_or_else(|| args.workspace_config.show_stderr.clone());
                args.task_result_sender
                    .send(TaskJobMessage::Started {
                        task: clean.job_id.clone(),
                        stdin_ready: stdin_ready.clone(),
                        show_stdout,
                        show_stderr,
                    })
                    .unwrap();
                execute_clean_job(
                    &args.workspace_config,
                    &lua,
                    &args.db_env,
                    &args.db,
                    &clean,
                    &args.task_result_sender,
                    &args.cache,
                );
            }
            ExecutorJob::ToolCheck(tool_check) => {
                args.task_result_sender
                    .send(TaskJobMessage::Started {
                        task: tool_check.job_id.clone(),
                        stdin_ready: stdin_ready.clone(),
                        show_stdout: args.workspace_config.show_stdout.clone(),
                        show_stderr: args.workspace_config.show_stderr.clone(),
                    })
                    .unwrap();
                execute_tool_check_job(
                    &args.workspace_config.workspace_dir,
                    &lua,
                    &tool_check,
                    &args.db_env,
                    &args.db,
                    &args.cache,
                    &args.task_result_sender,
                );
            }
            ExecutorJob::EnvAction(env_action_job) => {
                args.task_result_sender
                    .send(TaskJobMessage::Started {
                        task: env_action_job.job_id.clone(),
                        stdin_ready: stdin_ready.clone(),
                        show_stdout: args.workspace_config.show_stdout.clone(),
                        show_stderr: args.workspace_config.show_stderr.clone(),
                    })
                    .unwrap();
                execute_env_action_job(
                    &lua,
                    &args.db_env,
                    &args.db,
                    &env_action_job,
                    &stdin_ready,
                    &args.task_result_sender,
                    &args.cache,
                );
            }
        };
    }
}
