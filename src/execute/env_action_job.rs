// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{collections::HashMap, sync::{mpsc::Sender, Arc, Condvar, Mutex}};

use crate::{execute::{action::{create_action_context, invoke_action_protected, ActionContextArgs}, execute::{EnvActionJob, TaskExecutionError, TaskExecutorCache, TaskJobMessage, TaskResult}}, lua::lua_env::COBBLE_JOB_INTERACTIVE_ENABLED};

fn execute_env_action(
    lua: &mlua::Lua,
    job: &EnvActionJob,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    stdin_ready: &Arc<(Mutex<bool>, Condvar)>,
    sender: &Sender<TaskJobMessage>,
) -> Result<(), TaskExecutionError> {
    let project_dir = job.env.dir.to_str()
        .ok_or_else(|| TaskExecutionError::ExecutorError(format!("Unable to convert path to a string: {}", job.env.dir.display())))?;

    let args_strings: Vec<String> = job.args.iter().map(|s| String::from(s.as_ref())).collect();
    let args_val = lua.pack(args_strings)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let action_context = create_action_context(
        lua,
        ActionContextArgs {
            task_name: job.job_id.clone(),
            action: job.env.action.clone(),
            extra_tools: HashMap::new(),
            extra_envs: HashMap::new(),
            files: HashMap::new(),
            vars: HashMap::new(),
            task_outputs: HashMap::new(),
            project_dir: project_dir.to_owned(),
            args: args_val,
            workspace: job.workspace.clone(),
            db_env: db_env.clone(),
            db: db.clone(),
            cache: cache.clone(),
            sender: sender.clone(),
        }
    ).map_err(|e| TaskExecutionError::LuaError(e))?;


    let (ready_lock, ready_condvar) = stdin_ready.as_ref();
    let mut ready = ready_lock.lock().unwrap();
    while !*ready {
        ready = ready_condvar.wait(ready).unwrap();
    }
    lua.set_named_registry_value(COBBLE_JOB_INTERACTIVE_ENABLED, true)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let result_res = invoke_action_protected(lua, &job.env.action, action_context, true);

    lua.set_named_registry_value(COBBLE_JOB_INTERACTIVE_ENABLED, true)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    result_res.and(Ok(()))
}


pub fn execute_env_action_job(
    lua: &mlua::Lua,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    job: &EnvActionJob,
    stdin_ready: &Arc<(Mutex<bool>, Condvar)>,
    task_result_sender: &Sender<TaskJobMessage>,
    cache: &Arc<TaskExecutorCache>,
) {
    let result = execute_env_action(lua, job, db_env, db, cache, stdin_ready, &task_result_sender);

    match result {
        Ok(_) => {
            task_result_sender.send(TaskJobMessage::Complete{
                task: job.job_id.clone(),
                result: TaskResult::Success
            }).unwrap();
        }
        Err(e) => {
            task_result_sender.send(TaskJobMessage::Complete {
                task: job.job_id.clone(),
                result: TaskResult::Error(e)
            }).unwrap();
        }
    }
}