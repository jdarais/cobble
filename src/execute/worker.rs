use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use crate::config::WorkspaceConfig;
use crate::execute::action::init_lua_for_task_executor;
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

        match next_task {
            ExecutorJob::Task(task) => {
                execute_task_job(
                    &args.workspace_config,
                    &lua,
                    args.db_env.as_ref(),
                    &args.db,
                    &task,
                    args.task_result_sender.clone(),
                    args.cache.clone(),
                );
            }
            ExecutorJob::ToolCheck(tool_check) => {
                execute_tool_check_job(
                    &args.workspace_config.workspace_dir,
                    &lua,
                    &tool_check,
                    &args.task_result_sender,
                );
            }
        };
    }
}
