use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::execute::execute::{TaskExecutionError, TaskJobMessage};
use crate::project_def::types::{json_to_lua, TaskVar};
use crate::project_def::{Action, ExternalTool};
use crate::workspace::Workspace;

#[derive(Clone)]
pub struct ActionContextFile {
    pub hash: String,
    pub path: String,
}

pub struct ActionContextArgs<'lua> {
    pub extra_tools: HashMap<String, String>,
    pub extra_envs: HashMap<String, String>,
    pub files: HashMap<String, ActionContextFile>,
    pub vars: HashMap<String, TaskVar>,
    pub task_outputs: HashMap<String, serde_json::Value>,
    pub project_dir: String,
    pub args: mlua::Value<'lua>,
    pub sender: Sender<TaskJobMessage>,
}

pub fn init_lua_for_task_executor(lua: &mlua::Lua) -> mlua::Result<()> {
    let task_executor_env_source = include_bytes!("task_executor.lua");
    lua.load(&task_executor_env_source[..]).exec()
}

pub fn execute_action<'lua>(
    lua: &'lua mlua::Lua,
    task_name: &Arc<str>,
    action: &Action,
    context_args: ActionContextArgs<'lua>,
) -> Result<mlua::Value<'lua>, TaskExecutionError> {
    let (success, result) = execute_action_xpcall(lua, task_name, action, context_args)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    if success {
        return Ok(result);
    } else {
        let message = match result {
            mlua::Value::String(s) => s.to_str().unwrap_or("<error reading message>").to_owned(),
            mlua::Value::Error(e) => e.to_string(),
            _ => format!("{:?}", result),
        };
        return Err(TaskExecutionError::ActionFailed(message));
    }
}

pub fn execute_action_xpcall<'lua>(
    lua: &'lua mlua::Lua,
    task_name: &Arc<str>,
    action: &Action,
    context_args: ActionContextArgs<'lua>,
) -> mlua::Result<(bool, mlua::Value<'lua>)> {
    let ActionContextArgs {
        extra_tools,
        extra_envs,
        files,
        vars,
        task_outputs,
        project_dir,
        args,
        sender,
    } = context_args;
    let out_task_name_clone = task_name.clone();
    let out_sender_clone = sender.clone();
    let out = lua.create_function(move |_lua, s: String| {
        out_sender_clone
            .send(TaskJobMessage::Stdout {
                task: out_task_name_clone.clone(),
                s,
            })
            .map_err(|e| {
                mlua::Error::runtime(format!("Error sending output from executor thread: {}", e))
            })
    })?;

    let err_task_name_clone = task_name.clone();
    let err_sender_clone = sender.clone();
    let err = lua.create_function(move |_lua, s: String| {
        err_sender_clone
            .send(TaskJobMessage::Stderr {
                task: err_task_name_clone.clone(),
                s,
            })
            .map_err(|e| {
                mlua::Error::runtime(format!("Error sending output from executor thread: {}", e))
            })
    })?;

    let action_lua = lua.pack(action.clone())?;

    let task_outputs_lua = lua.create_table().and_then(|tbl| {
        for (k, v) in task_outputs.iter() {
            tbl.set(k.clone(), json_to_lua(lua, v.clone())?)?;
        }
        Ok(tbl)
    })?;

    // let file_hashes = task_inputs.file_hashes.clone();
    let files_lua = lua.create_table().and_then(|tbl| {
        for (k, v) in files.into_iter() {
            let file_tbl = lua.create_table()?;
            let ActionContextFile { path, hash } = v;
            file_tbl.set("path", path)?;
            file_tbl.set("hash", hash)?;
            tbl.set(k.clone(), file_tbl)?;
        }
        Ok(tbl)
    })?;

    let cobble_table: mlua::Table = lua.globals().get("cobble")?;

    let create_action_context: mlua::Function = cobble_table.get("create_action_context")?;

    let action_context: mlua::Table = create_action_context.call((
        action_lua.clone(),
        extra_tools,
        extra_envs,
        files_lua,
        vars,
        task_outputs_lua,
        project_dir,
        out,
        err,
        args,
    ))?;

    let invoke_action_chunk = lua.load(
        r#"
        local action, action_context = ...
        return xpcall(cobble.invoke_action, function (msg) return msg end, action, action_context)
    "#,
    );

    let action_result: mlua::MultiValue = invoke_action_chunk.call((action_lua, action_context))?;

    let mut action_result_iter = action_result.into_iter();
    let success = action_result_iter.next().unwrap_or(mlua::Value::Nil);
    let result = action_result_iter.next().unwrap_or(mlua::Value::Nil);

    let success_bool: bool = lua.unpack(success)?;
    Ok((success_bool, result))
}

pub fn ensure_tool_is_cached(
    lua: &mlua::Lua,
    tool_name: &str,
    workspace: &Workspace,
) -> mlua::Result<()> {
    let tool = workspace
        .tools
        .get(tool_name)
        .ok_or_else(|| mlua::Error::runtime(format!("Tool lookup failed: {}", tool_name)))?;

    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let cached_tools: mlua::Table = cobble_table.get("_tool_cache")?;
    if !cached_tools.contains_key(tool.name.as_ref())? {
        cached_tools.set(tool.name.as_ref(), ExternalTool::clone(tool))?;
    }

    for (_, t_name) in tool.action.tools.iter() {
        ensure_tool_is_cached(lua, t_name.as_ref(), workspace)?;
    }

    Ok(())
}

pub fn ensure_build_env_is_cached(
    lua: &mlua::Lua,
    build_env_name: &str,
    workspace: &Workspace,
) -> mlua::Result<()> {
    let build_env = workspace.build_envs.get(build_env_name).ok_or_else(|| {
        mlua::Error::runtime(format!("Build env lookup failed: {}", build_env_name))
    })?;

    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let cached_build_envs: mlua::Table = cobble_table.get("_build_env_cache")?;
    if !cached_build_envs.contains_key(build_env.name.as_ref())? {
        let build_env_table = lua.create_table()?;
        build_env_table.set("action", build_env.action.clone())?;
        cached_build_envs.set(build_env.name.as_ref(), build_env_table)?;
    }

    for (_, t_name) in build_env.action.tools.iter() {
        ensure_tool_is_cached(lua, t_name, workspace)?;
    }

    for (_, e_name) in build_env.action.build_envs.iter() {
        ensure_build_env_is_cached(lua, e_name, workspace)?;
    }

    Ok(())
}
