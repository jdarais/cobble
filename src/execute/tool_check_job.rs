use std::sync::mpsc::Sender;
use std::{collections::HashMap, path::Path};

use crate::execute::{
    action::{ensure_tool_is_cached, execute_action, ActionContextArgs},
    execute::{TaskExecutionError, TaskJobMessage, TaskResult, ToolCheckJob},
};

pub fn execute_tool_check_job(
    workspace_dir: &Path,
    lua: &mlua::Lua,
    job: &ToolCheckJob,
    sender: &Sender<TaskJobMessage>,
) {
    let result = execute_tool_check_action(workspace_dir, lua, job, sender);

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
    sender: &Sender<TaskJobMessage>,
) -> Result<(), TaskExecutionError> {
    let check_action = match &job.tool.check {
        Some(action) => action,
        None => {
            return Ok(());
        }
    };

    ensure_tool_is_cached(lua, job.tool_name.as_ref(), &job.workspace)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    for tool_name in check_action.tools.values() {
        ensure_tool_is_cached(lua, tool_name.as_ref(), &job.workspace)
            .map_err(|e| TaskExecutionError::LuaError(e))?;
    }

    execute_action(
        lua,
        &job.job_id,
        &check_action,
        ActionContextArgs {
            extra_envs: HashMap::new(),
            extra_tools: HashMap::new(),
            files: HashMap::new(),
            vars: HashMap::new(),
            task_outputs: HashMap::new(),
            // TODO: Tools should be global, so it shouldn't matter what directory
            // this is set to, but it would be nicer to set it to the project that
            // the tool was defined in
            project_dir: String::from(workspace_dir.to_str().unwrap()),
            args: mlua::Value::Nil,
            sender: sender.clone(),
        },
    )?;

    Ok(())
}
