// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::db::{TaskInput, TaskOutput};
use crate::execute::execute::{TaskExecutionError, TaskExecutorCache, TaskJobMessage};
use crate::project_def::types::{json_to_lua, StringOrInt};
use crate::project_def::Action;
use crate::vars::extract_vars;
use crate::workspace::{BuildEnv, Task, Workspace};

#[derive(Clone)]
pub struct ActionContextFile {
    pub hash: String,
    pub path: String,
}

pub struct ActionContextArgs<'lua> {
    pub task_name: Arc<str>,
    pub action: Action,
    pub extra_tools: BTreeMap<Arc<str>, Arc<str>>,
    pub extra_envs: BTreeMap<Arc<str>, Arc<str>>,
    pub files: BTreeMap<StringOrInt, ActionContextFile>,
    pub var_deps: Vec<Arc<str>>,
    pub all_vars: Arc<serde_json::Map<String, serde_json::Value>>,
    pub task_outputs: BTreeMap<StringOrInt, TaskOutput>,
    pub project_dir: String,
    pub args: mlua::Value<'lua>,
    pub workspace: Arc<Workspace>,
    pub cache: Arc<TaskExecutorCache>,
    pub sender: Sender<TaskJobMessage>,
}

fn get_original_error(error: &mlua::Error) -> &mlua::Error {
    match error {
        mlua::Error::CallbackError {
            traceback: _,
            cause,
        } => get_original_error(cause.as_ref()),
        mlua::Error::WithContext { context: _, cause } => get_original_error(cause.as_ref()),
        e => e,
    }
}

fn get_error_message(val: &mlua::Value) -> String {
    match val {
        mlua::Value::String(s) => s.to_str().unwrap_or("<error reading message>").to_owned(),
        mlua::Value::Error(e) => get_original_error(&e).to_string(),
        _ => format!("{:?}", val),
    }
}

pub fn init_lua_for_task_executor(lua: &mlua::Lua) -> mlua::Result<()> {
    let task_executor_env_source = include_bytes!("task_executor.lua");
    lua.load(&task_executor_env_source[..]).exec()
}

pub fn invoke_action_protected<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    action_context: mlua::Table<'lua>,
    return_arg_list_action_result: bool,
) -> Result<mlua::Value<'lua>, TaskExecutionError> {
    let (success, result) =
        execute_action_pcall(lua, action, action_context, return_arg_list_action_result)
            .map_err(|e| TaskExecutionError::LuaError(e))?;

    if success {
        return Ok(result);
    } else {
        let message = get_error_message(&result);
        return Err(TaskExecutionError::ActionFailed(message));
    }
}

fn invoke_tool_by_name<'lua>(
    lua: &'lua mlua::Lua,
    tool_name: &Arc<str>,
    task_name: &Arc<str>,
    all_vars: Arc<serde_json::Map<String, serde_json::Value>>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Value<'lua>> {
    let tool = workspace.tools.get(tool_name).ok_or_else(|| {
        mlua::Error::runtime(format!(
            "Tried to invoke tool '{}', but no tool with that name exists.",
            tool_name
        ))
    })?;

    let tool_action = &tool.action;
    let action_context = create_tool_action_context(
        lua,
        tool_action,
        task_name,
        tool.var_deps.clone(),
        all_vars,
        project_dir,
        args,
        workspace,
        cache,
        task_event_sender,
    )?;
    let (success, result) = execute_action_pcall(lua, tool_action, action_context, true)?;

    if success {
        Ok(result)
    } else {
        Err(mlua::Error::external(get_error_message(&result)))
    }
}

fn invoke_env_by_name<'lua>(
    lua: &'lua mlua::Lua,
    env_name: &Arc<str>,
    task_name: &Arc<str>,
    vars: Arc<serde_json::Map<String, serde_json::Value>>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Value<'lua>> {
    let env = workspace.build_envs.get(env_name).ok_or_else(|| {
        mlua::Error::runtime(format!(
            "Tried to invoke env '{}', but no env with that name exists.",
            env_name
        ))
    })?;

    let env_action = &env.action;
    let action_context = create_env_action_context(
        lua,
        env_action,
        env,
        task_name,
        env.var_deps.clone(),
        vars,
        project_dir,
        args,
        workspace,
        cache,
        task_event_sender,
    )?;
    let (success, result) = execute_action_pcall(lua, env_action, action_context, true)?;

    if success {
        Ok(result)
    } else {
        Err(mlua::Error::external(get_error_message(&result)))
    }
}

pub fn create_tool_action_context<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    task_name: &Arc<str>,
    var_deps: Vec<Arc<str>>,
    all_vars: Arc<serde_json::Map<String, serde_json::Value>>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task_name.clone(),
            action: action.clone(),
            extra_tools: Default::default(),
            extra_envs: Default::default(),
            files: Default::default(),
            var_deps,
            all_vars,
            task_outputs: Default::default(),
            project_dir,
            args,
            workspace: workspace.clone(),
            cache: cache.clone(),
            sender: task_event_sender.clone(),
        },
    )
}

pub fn create_env_action_context<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    env: &Arc<BuildEnv>,
    task_name: &Arc<str>,
    var_deps: Vec<Arc<str>>,
    all_vars: Arc<serde_json::Map<String, serde_json::Value>>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    let env_setup_task_output_opt = match env.setup_task.as_ref() {
        Some(setup_task) => cache
            .task_outputs
            .read()
            .unwrap()
            .get(setup_task)
            .map(|o| Some(o.clone()))
            .ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Unable to retrieve output of setup task for env: {}",
                    env.name.as_ref()
                ))
            })?,
        None => None,
    };

    let mut task_outputs: BTreeMap<StringOrInt, TaskOutput> = BTreeMap::new();

    if let Some(env_setup_task_output) = env_setup_task_output_opt {
        task_outputs.insert(StringOrInt::String(Arc::from("setup_task")), env_setup_task_output);
    }

    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task_name.clone(),
            action: action.clone(),
            extra_tools: Default::default(),
            extra_envs: Default::default(),
            files: Default::default(),
            var_deps,
            all_vars,
            task_outputs,
            project_dir,
            args,
            workspace: workspace.clone(),
            cache: cache.clone(),
            sender: task_event_sender.clone(),
        },
    )
}

pub fn create_task_action_context<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    task: &Arc<Task>,
    task_input: &TaskInput,
    args: mlua::Value<'lua>,
    all_vars: Arc<serde_json::Map<String, serde_json::Value>>,
    workspace: &Arc<Workspace>,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    let mut files: BTreeMap<StringOrInt, ActionContextFile> = BTreeMap::new();
    for (file_alias, file_dep) in &task.file_deps {
        let hash = task_input
            .file_hashes
            .get(file_dep.path.as_ref())
            .ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Expected file hash to be available for {}: {}, but it is missing",
                    file_alias, file_dep.path
                ))
            })?;

        files.insert(
            file_alias.clone(),
            ActionContextFile {
                hash: hash.clone(),
                path: file_dep.path.to_string(),
            },
        );
    }

    let project_dir = task.dir.to_str().map(|s| s.to_owned()).ok_or_else(|| {
        mlua::Error::runtime(format!(
            "Error converting path to s a string: {}",
            task.dir.display()
        ))
    })?;

    let mut task_outputs: BTreeMap<StringOrInt, TaskOutput> = BTreeMap::new();

    {
        let cached_outputs_read = cache.task_outputs.read().unwrap();
        for (task_alias, task_dep) in task.task_deps.iter() {
            let task_dep_output = cached_outputs_read.get(task_dep).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Expected output for task {} to be available, but it is missing",
                    task_dep
                ))
            })?;

            task_outputs.insert(task_alias.clone(), task_dep_output.clone());
        }
    }

    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task.name.clone(),
            action: action.clone(),
            extra_tools: task.tools.clone(),
            extra_envs: task.build_envs.clone(),
            files,
            var_deps: task.var_deps.clone(),
            all_vars,
            task_outputs,
            project_dir,
            args,
            workspace: workspace.clone(),
            cache: cache.clone(),
            sender: task_event_sender.clone(),
        },
    )
}

pub fn create_action_context<'lua>(
    lua: &'lua mlua::Lua,
    context_args: ActionContextArgs,
) -> mlua::Result<mlua::Table<'lua>> {
    let ActionContextArgs {
        task_name,
        action,
        extra_tools,
        extra_envs,
        files,
        var_deps,
        all_vars,
        task_outputs,
        project_dir,
        args,
        workspace,
        cache,
        sender,
    } = context_args;

    let action_context = lua.create_table()?;

    let out_task_name_clone = task_name.clone();
    let out_sender_clone = sender.clone();
    let print_fn = lua.create_function(move |_lua, s: String| {
        out_sender_clone
            .send(TaskJobMessage::Stdout {
                task: out_task_name_clone.clone(),
                s,
            })
            .map_err(|e| {
                mlua::Error::runtime(format!("Error sending output from executor thread: {}", e))
            })
    })?;
    action_context.set("print", print_fn.clone())?;

    let println_fn: mlua::Function = lua
        .load(
            r#"
        local print_fn = ...
        return function (s) print_fn(s.."\n") end    
    "#,
        )
        .call(print_fn)?;
    action_context.set("println", println_fn)?;

    let err_task_name_clone = task_name.clone();
    let err_sender_clone = sender.clone();
    let eprint_fn = lua.create_function(move |_lua, s: String| {
        err_sender_clone
            .send(TaskJobMessage::Stderr {
                task: err_task_name_clone.clone(),
                s,
            })
            .map_err(|e| {
                mlua::Error::runtime(format!("Error sending output from executor thread: {}", e))
            })
    })?;
    action_context.set("eprint", eprint_fn.clone())?;

    let eprintln_fn: mlua::Function = lua
        .load(
            r#"
        local eprint_fn = ...
        return function (s) eprint_fn(s.."\n") end
    "#,
        )
        .call(eprint_fn)?;
    action_context.set("eprintln", eprintln_fn)?;

    let tool_table = lua.create_table()?;
    for (tool_alias, tool_name) in extra_tools.iter().chain(action.tools.iter()) {
        let tool_name_clone = tool_name.clone();
        let task_name_clone = task_name.clone();
        let all_vars_clone = all_vars.clone();
        let project_dir_clone = project_dir.clone();
        let workspace_clone = workspace.clone();
        let cache_clone = cache.clone();
        let sender_clone = sender.clone();
        let invoke_tool_fn = lua.create_function(move |fn_lua, args: mlua::Value| {
            invoke_tool_by_name(
                fn_lua,
                &tool_name_clone,
                &task_name_clone,
                all_vars_clone.clone(),
                project_dir_clone.clone(),
                args,
                &workspace_clone,
                &cache_clone,
                &sender_clone,
            )
        })?;
        tool_table.set(tool_alias.to_string(), invoke_tool_fn)?;
    }
    action_context.set("tool", tool_table)?;

    let env_table = lua.create_table()?;
    for (env_alias, env_name) in extra_envs.iter().chain(action.build_envs.iter()) {
        let env_name_clone = env_name.clone();
        let task_name_clone = task_name.clone();
        let all_vars_clone = all_vars.clone();
        let project_dir_clone = project_dir.clone();
        let workspace_clone = workspace.clone();
        let cache_clone = cache.clone();
        let sender_clone = sender.clone();
        let invoke_env_fn = lua.create_function(move |fn_lua, args| {
            // TODO: Avoid the double-clone here
            invoke_env_by_name(
                fn_lua,
                &env_name_clone,
                &task_name_clone,
                all_vars_clone.clone(),
                project_dir_clone.clone(),
                args,
                &workspace_clone,
                &cache_clone,
                &sender_clone,
            )
        })?;
        env_table.set(env_alias.to_string(), invoke_env_fn)?;
    }
    action_context.set("env", env_table)?;

    action_context.set("action", lua.pack(action)?)?;

    let task_outputs_lua = lua.create_table().and_then(|tbl| {
        for (k, v) in task_outputs.iter() {
            let task_tbl = lua.create_table()?;

            let task_files_tbl = lua.create_table()?;
            for (path, hash) in v.file_hashes.iter() {
                let file_tbl = lua.create_table()?;
                file_tbl.set("path", path.clone())?;
                file_tbl.set("hash", hash.clone())?;
                // TODO: allow named artifacts and use the artifact alias here
                task_files_tbl.set(path.clone(), file_tbl)?;
            }
            task_tbl.set("files", task_files_tbl)?;
            task_tbl.set("output", v.task_output.clone())?;

            tbl.set(k.clone(), task_tbl)?;
        }
        Ok(tbl)
    })?;
    action_context.set("tasks", task_outputs_lua)?;

    let files_lua = lua.create_table().and_then(|tbl| {
        for (k, v) in files.into_iter() {
            let file_tbl = lua.create_table()?;
            let ActionContextFile { path, hash } = v;
            file_tbl.set("path", path)?;
            file_tbl.set("hash", hash)?;
            tbl.set(k, file_tbl)?;
        }
        Ok(tbl)
    })?;
    action_context.set("files", files_lua)?;

    let mut vars: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    extract_vars(var_deps.iter(), all_vars.as_ref(), &mut vars).map_err(|e| {
        mlua::Error::runtime(format!("Var lookup for task {} failed: {}", &task_name, e))
    })?;

    action_context.set(
        "vars",
        json_to_lua(lua, serde_json::Value::Object(vars))?,
    )?;

    let project_table = lua.create_table()?;
    project_table.set("dir", project_dir)?;
    action_context.set("project", project_table)?;

    action_context.set("args", args)?;

    Ok(action_context)
}

pub fn execute_action_pcall<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    action_context: mlua::Table<'lua>,
    return_arg_list_action_result: bool,
) -> mlua::Result<(bool, mlua::Value<'lua>)> {
    let invoke_action_source = include_bytes!("invoke_action.lua");
    let invoke_action_fn = lua.load(&invoke_action_source[..]);

    let action_result: mlua::MultiValue = invoke_action_fn.call((
        action.clone(),
        action_context,
        return_arg_list_action_result,
    ))?;

    let mut action_result_iter = action_result.into_iter();
    let success = action_result_iter.next().unwrap_or(mlua::Value::Nil);
    let result = action_result_iter.next().unwrap_or(mlua::Value::Nil);

    let success_bool: bool = lua.unpack(success)?;
    Ok((success_bool, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_original_error_from_callback_error() {
        let original_error = Arc::new(mlua::Error::external(String::from("test error")));

        let error = mlua::Error::CallbackError {
            traceback: String::from("test traceback"),
            cause: original_error.clone(),
        };

        assert_eq!(
            get_original_error(&error).to_string(),
            original_error.to_string()
        );
    }

    #[test]
    fn test_get_error_message_unwraps_error_chain() {
        let original_error = Arc::new(mlua::Error::external(String::from("test error")));

        let error = mlua::Error::CallbackError {
            traceback: String::from("test traceback"),
            cause: original_error.clone(),
        };

        assert_eq!(
            get_error_message(&mlua::Value::Error(error)),
            original_error.to_string()
        )
    }
}
