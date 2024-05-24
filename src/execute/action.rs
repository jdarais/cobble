use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::db::{get_task_record, TaskInput};
use crate::execute::execute::{TaskExecutionError, TaskExecutorCache, TaskJobMessage};
use crate::project_def::types::{json_to_lua, TaskVar};
use crate::project_def::{Action, BuildEnv};
use crate::workspace::{Task, Workspace};

#[derive(Clone)]
pub struct ActionContextFile {
    pub hash: String,
    pub path: String,
}

pub struct ActionContextArgs<'lua> {
    pub task_name: Arc<str>,
    pub action: Action,
    pub extra_tools: HashMap<Arc<str>, Arc<str>>,
    pub extra_envs: HashMap<Arc<str>, Arc<str>>,
    pub files: HashMap<Arc<str>, ActionContextFile>,
    pub vars: HashMap<String, TaskVar>,
    pub task_outputs: HashMap<String, serde_json::Value>,
    pub project_dir: String,
    pub args: mlua::Value<'lua>,
    pub workspace: Arc<Workspace>,
    pub db_env: Arc<lmdb::Environment>,
    pub db: lmdb::Database,
    pub cache: Arc<TaskExecutorCache>,
    pub sender: Sender<TaskJobMessage>,
}

fn get_error_message(val: &mlua::Value) -> String {
    match val {
        mlua::Value::String(s) => s.to_str().unwrap_or("<error reading message>").to_owned(),
        mlua::Value::Error(e) => e.to_string(),
        _ => format!("{:?}", val),
    }
}

pub fn init_lua_for_task_executor(lua: &mlua::Lua) -> mlua::Result<()> {
    let task_executor_env_source = include_bytes!("task_executor.lua");
    lua.load(&task_executor_env_source[..]).exec()
}

// pub fn invoke_action<'lua>(
//     lua: &'lua mlua::Lua,
//     action: &Action,
//     action_context: mlua::Table<'lua>,
// ) -> mlua::Result<mlua::Value<'lua>> {
//     let invoke_action_source = include_bytes!("invoke_action.lua");
//     let invoke_action_fn: mlua::Function = lua.load(&invoke_action_source[..]).eval()?;
//     invoke_action_fn.call((action.clone(), action_context))
// }

pub fn invoke_action_protected<'lua>(
    lua: &'lua mlua::Lua,
    action: &Action,
    action_context: mlua::Table<'lua>,
) -> Result<mlua::Value<'lua>, TaskExecutionError> {
    let (success, result) = execute_action_pcall(lua, action, action_context)
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
    files: HashMap<Arc<str>, ActionContextFile>,
    vars: HashMap<String, TaskVar>,
    task_outputs: HashMap<String, serde_json::Value>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
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
        files,
        vars,
        task_outputs,
        project_dir,
        args,
        workspace,
        db_env,
        db,
        cache,
        task_event_sender,
    )?;
    let (success, result) = execute_action_pcall(lua, tool_action, action_context)?;

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
    files: HashMap<Arc<str>, ActionContextFile>,
    vars: HashMap<String, TaskVar>,
    task_outputs: HashMap<String, serde_json::Value>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
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
        files,
        vars,
        task_outputs,
        project_dir,
        args,
        workspace,
        db_env,
        db,
        cache,
        task_event_sender,
    )?;
    let (success, result) = execute_action_pcall(lua, env_action, action_context)?;

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
    files: HashMap<Arc<str>, ActionContextFile>,
    vars: HashMap<String, TaskVar>,
    task_outputs: HashMap<String, serde_json::Value>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task_name.clone(),
            action: action.clone(),
            extra_tools: HashMap::new(),
            extra_envs: HashMap::new(),
            files: files,
            vars: vars,
            task_outputs: task_outputs,
            project_dir,
            args,
            workspace: workspace.clone(),
            db_env: db_env.clone(),
            db: db.clone(),
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
    files: HashMap<Arc<str>, ActionContextFile>,
    vars: HashMap<String, TaskVar>,
    task_outputs: HashMap<String, serde_json::Value>,
    project_dir: String,
    args: mlua::Value<'lua>,
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    let env_install_task_output_opt = cache
        .task_outputs
        .read()
        .unwrap()
        .get(&env.name)
        .map(|v| v.clone());
    let env_install_task_output = match env_install_task_output_opt {
        Some(output) => output,
        None => match get_task_record(db_env, db.clone(), &env.name) {
            Ok(record) => record.output.task_output,
            Err(e) => {
                return Err(mlua::Error::runtime(format!(
                    "Unable to retrieve output for env install task {}: {}",
                    env.name, e
                )));
            }
        },
    };

    let mut task_outputs_with_install: HashMap<String, serde_json::Value> = task_outputs;
    let mut existing_opt =
        task_outputs_with_install.insert(String::from("install"), env_install_task_output);

    let mut existing_install_prefix = String::from("_");
    while let Some(existing) = existing_opt.take() {
        let mut prefixed_key = existing_install_prefix.clone();
        prefixed_key.push_str("install");
        existing_opt = task_outputs_with_install.insert(prefixed_key, existing);
        existing_install_prefix.push_str("_");
    }

    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task_name.clone(),
            action: action.clone(),
            extra_tools: HashMap::new(),
            extra_envs: HashMap::new(),
            files: files,
            vars: vars,
            task_outputs: task_outputs_with_install,
            project_dir,
            args,
            workspace: workspace.clone(),
            db_env: db_env.clone(),
            db: db.clone(),
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
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    task_event_sender: &Sender<TaskJobMessage>,
) -> mlua::Result<mlua::Table<'lua>> {
    let mut files: HashMap<Arc<str>, ActionContextFile> = HashMap::new();
    for (file_alias, file_dep) in &task.file_deps {
        let hash = task_input
            .file_hashes
            .get(file_alias.as_ref())
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

    create_action_context(
        lua,
        ActionContextArgs {
            task_name: task.name.clone(),
            action: action.clone(),
            extra_tools: task.tools.clone(),
            extra_envs: task.build_envs.clone(),
            files,
            vars: task_input.vars.clone(),
            task_outputs: task_input.task_outputs.clone(),
            project_dir,
            args,
            workspace: workspace.clone(),
            db_env: db_env.clone(),
            db: db.clone(),
            cache: cache.clone(),
            sender: task_event_sender.clone(),
        },
    )
}

fn create_action_context<'lua>(
    lua: &'lua mlua::Lua,
    context_args: ActionContextArgs,
) -> mlua::Result<mlua::Table<'lua>> {
    let ActionContextArgs {
        task_name,
        action,
        extra_tools,
        extra_envs,
        files,
        vars,
        task_outputs,
        project_dir,
        args,
        workspace,
        db_env,
        db,
        cache,
        sender,
    } = context_args;

    let action_context = lua.create_table()?;

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
    action_context.set("out", out)?;

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
    action_context.set("err", err)?;

    let tool_table = lua.create_table()?;
    for (tool_alias, tool_name) in extra_tools.iter().chain(action.tools.iter()) {
        let tool_name_clone = tool_name.clone();
        let task_name_clone = task_name.clone();
        let files_clone = files.clone();
        let vars_clone = vars.clone();
        let task_outputs_clone = task_outputs.clone();
        let project_dir_clone = project_dir.clone();
        let workspace_clone = workspace.clone();
        let db_env_clone = db_env.clone();
        let db_clone = db.clone();
        let cache_clone = cache.clone();
        let sender_clone = sender.clone();
        let invoke_tool_fn = lua.create_function(move |fn_lua, args: mlua::Value| {
            invoke_tool_by_name(
                fn_lua,
                &tool_name_clone,
                &task_name_clone,
                files_clone.clone(),
                vars_clone.clone(),
                task_outputs_clone.clone(),
                project_dir_clone.clone(),
                args,
                &workspace_clone,
                &db_env_clone,
                &db_clone,
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
        let files_clone = files.clone();
        let vars_clone = vars.clone();
        let task_outputs_clone = task_outputs.clone();
        let project_dir_clone = project_dir.clone();
        let workspace_clone = workspace.clone();
        let db_env_clone = db_env.clone();
        let db_clone = db.clone();
        let cache_clone = cache.clone();
        let sender_clone = sender.clone();
        let invoke_env_fn = lua.create_function(move |fn_lua, args| {
            // TODO: Avoid the double-clone here
            invoke_env_by_name(
                fn_lua,
                &env_name_clone,
                &task_name_clone,
                files_clone.clone(),
                vars_clone.clone(),
                task_outputs_clone.clone(),
                project_dir_clone.clone(),
                args,
                &workspace_clone,
                &db_env_clone,
                &db_clone,
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
            tbl.set(k.clone(), json_to_lua(lua, v.clone())?)?;
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
            tbl.set(k.to_string(), file_tbl)?;
        }
        Ok(tbl)
    })?;
    action_context.set("files", files_lua)?;

    action_context.set("vars", vars)?;

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
) -> mlua::Result<(bool, mlua::Value<'lua>)> {
    let invoke_action_source = include_bytes!("invoke_action.lua");
    let invoke_action_fn: mlua::Function = lua.load(&invoke_action_source[..]).eval()?;

    let action_result: mlua::MultiValue = invoke_action_fn.call((action.clone(), action_context))?;

    let mut action_result_iter = action_result.into_iter();
    let success = action_result_iter.next().unwrap_or(mlua::Value::Nil);
    let result = action_result_iter.next().unwrap_or(mlua::Value::Nil);

    let success_bool: bool = lua.unpack(success)?;
    Ok((success_bool, result))
}
