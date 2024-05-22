use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::{collections::HashMap, path::Path};

use crate::execute::action::{create_tool_action_context, invoke_action, invoke_action_protected};
use crate::execute::execute::TaskExecutorCache;
use crate::execute::{
    action::{ensure_tool_is_cached, ActionContextArgs},
    execute::{TaskExecutionError, TaskJobMessage, TaskResult, ToolCheckJob},
};
use crate::workspace::Workspace;

pub fn execute_tool_check_job(
    workspace_dir: &Path,
    lua: &mlua::Lua,
    job: &ToolCheckJob,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    sender: &Sender<TaskJobMessage>,
) {
    let result = execute_tool_check_action(workspace_dir, lua, job, db_env, db, cache, sender);

    match result {
        Ok(_) => {
            sender
                .send(TaskJobMessage::Complete {
                    task: job.job_id.clone(),
                    result: TaskResult::Success,
                })
                .unwrap();
        }
        Err(e) => {
            sender
                .send(TaskJobMessage::Complete {
                    task: job.job_id.clone(),
                    result: TaskResult::Error(e),
                })
                .unwrap();
        }
    }
}

fn execute_tool_check_action(
    workspace_dir: &Path,
    lua: &mlua::Lua,
    job: &ToolCheckJob,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    sender: &Sender<TaskJobMessage>,
) -> Result<(), TaskExecutionError> {
    let check_action = match &job.tool.check {
        Some(action) => action,
        None => {
            return Ok(());
        }
    };

    let project_dir = workspace_dir.to_str().map(|s| s.to_owned())
        .ok_or_else(|| TaskExecutionError::ExecutorError(format!("Error converting path to string: {}", workspace_dir.display())))?;

    let action_context_res = create_tool_action_context(
        lua,
        check_action,
        &job.tool,
        &job.job_id,
        project_dir,
        mlua::Value::Nil,
        &job.workspace,
        db_env,
        db,
        cache,
        &sender
    );
    let action_context = action_context_res.map_err(|e| TaskExecutionError::LuaError(e))?;

    invoke_action_protected(lua, &check_action, action_context)?;

    Ok(())
}
