use std::{collections::HashMap, fs::remove_file, sync::{mpsc::Sender, Arc, Condvar, Mutex}};

use crate::{config::WorkspaceConfig, db::delete_task_record, execute::{action::{create_action_context, invoke_action_protected, ActionContextArgs}, execute::{CleanJob, TaskExecutionError, TaskExecutorCache, TaskJobMessage, TaskResult}}, project_def::types::TaskVar, vars::get_var};

fn execute_clean_actions(
    lua: &mlua::Lua,
    job: &CleanJob,
    workspace_config: &Arc<WorkspaceConfig>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    sender: &Sender<TaskJobMessage>,
) -> Result<(), TaskExecutionError> {
    let mut vars: HashMap<String, TaskVar> = workspace_config.vars.clone();
    for (var_alias, var_name) in &job.task.var_deps {
        let var = get_var(var_name.as_ref(), &workspace_config.vars)
            .map_err(|e| TaskExecutionError::VarLookupError(e))?;
        vars.insert(var_alias.as_ref().to_owned(), var.clone());
    }

    let project_dir = job.task.dir.to_str()
        .ok_or_else(|| TaskExecutionError::ExecutorError(format!("Unable to convert path to a string: {}", job.task.dir.display())))?;

    for action in &job.task.clean_actions {
        let action_context = create_action_context(
            lua,
            ActionContextArgs {
                task_name: job.job_id.clone(),
                action: action.clone(),
                extra_tools: job.task.tools.clone(),
                extra_envs: job.task.build_envs.clone(),
                files: HashMap::new(),
                vars: vars.clone(),
                task_outputs: HashMap::new(),
                project_dir: project_dir.to_owned(),
                args: mlua::Value::Nil,
                workspace: job.workspace.clone(),
                db_env: db_env.clone(),
                db: db.clone(),
                cache: cache.clone(),
                sender: sender.clone(),
            }
        ).map_err(|e| TaskExecutionError::LuaError(e))?;

        invoke_action_protected(lua, action, action_context)?;
    }

    // Delete artifacts
    for artifact in &job.task.artifacts {
        let file_path = workspace_config.workspace_dir.join(artifact.filename.as_ref());
        if file_path.is_file() {
            remove_file(&file_path).map_err(|e| {
                TaskExecutionError::ExecutorError(format!(
                    "Error deleting file '{}': {}",
                    file_path.display(),
                    e
                ))
            })?;
        }
    }

    delete_task_record(db_env.as_ref(), db.clone(), job.task.name.as_ref())
        .map_err(|e| TaskExecutionError::DBDeleteError(e) )?;

    Ok(())
}


pub fn execute_clean_job(
    workspace_config: &Arc<WorkspaceConfig>,
    lua: &mlua::Lua,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    job: &CleanJob,
    task_result_sender: &Sender<TaskJobMessage>,
    cache: &Arc<TaskExecutorCache>,
) {
    let result = execute_clean_actions(lua, job, workspace_config, db_env, db, cache, &task_result_sender);

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