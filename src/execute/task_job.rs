use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::time::SystemTime;

use crate::config::WorkspaceConfig;
use crate::db::{
    get_task_record, put_task_record, GetError, PutError, TaskInput, TaskOutput, TaskRecord,
};
use crate::execute::action::{create_task_action_context, invoke_action_protected};
use crate::execute::execute::{
    TaskExecutionError, TaskExecutorCache, TaskJob, TaskJobMessage, TaskResult,
};
use crate::lua::detached::DetachedLuaValue;
use crate::lua::lua_env::COBBLE_JOB_INTERACTIVE_ENABLED;
use crate::util::hash::compute_file_hash;
use crate::vars::get_var;
use crate::workspace::{Task, Workspace};

fn execute_task_actions<'lua>(
    lua: &'lua mlua::Lua,
    task: &TaskJob,
    task_inputs: &TaskInput,
    workspace: &Arc<Workspace>,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
    sender: &Sender<TaskJobMessage>,
) -> Result<mlua::Value<'lua>, TaskExecutionError> {
    let mut args: mlua::Value = mlua::Value::Nil;
    for action in task.task.actions.iter() {
        let action_context_res = create_task_action_context(
            lua,
            action,
            &task.task,
            task_inputs,
            args,
            workspace,
            db_env,
            db,
            cache,
            sender,
        );
        let action_context = action_context_res.map_err(|e| TaskExecutionError::LuaError(e))?;

        args = invoke_action_protected(lua, action, action_context)?;
    }

    Ok(args)
}

fn get_current_task_input(
    workspace_config: &WorkspaceConfig,
    task: &Arc<Task>,
    db_env: &lmdb::Environment,
    db: &lmdb::Database,
    cache: &Arc<TaskExecutorCache>,
) -> Result<TaskInput, TaskExecutionError> {
    let mut current_task_input = TaskInput {
        project_source_hashes: HashMap::new(),
        file_hashes: HashMap::new(),
        task_outputs: HashMap::new(),
        vars: HashMap::new(),
    };

    for project_source in task.project_source_deps.iter() {
        let cached_hash = cache
            .project_source_hashes
            .read()
            .unwrap()
            .get(project_source)
            .cloned();
        let current_hash = match cached_hash {
            Some(hash) => hash,
            None => {
                let file_path = workspace_config
                    .workspace_dir
                    .join(Path::new(project_source.as_ref()));
                let file_hash = compute_file_hash(&file_path.as_path()).map_err(|e| {
                    TaskExecutionError::IOError {
                        message: format!("Error reading file {}", file_path.display()),
                        cause: e,
                    }
                })?;
                cache
                    .project_source_hashes
                    .write()
                    .unwrap()
                    .insert(project_source.clone(), file_hash.clone());
                file_hash
            }
        };
        current_task_input
            .project_source_hashes
            .insert(String::from(project_source.as_ref()), current_hash);
    }

    for (file_alias, file_dep) in task.file_deps.iter() {
        let cached_hash = cache
            .file_hashes
            .read()
            .unwrap()
            .get(&file_dep.path)
            .cloned();
        let current_hash = match cached_hash {
            Some(hash) => hash,
            None => {
                let file_path = workspace_config
                    .workspace_dir
                    .join(Path::new(file_dep.path.as_ref()));
                let file_hash = compute_file_hash(file_path.as_path()).map_err(|e| {
                    TaskExecutionError::IOError {
                        message: format!("Task {}: Error reading file {}", task.name, file_path.display()),
                        cause: e,
                    }
                })?;
                cache
                    .file_hashes
                    .write()
                    .unwrap()
                    .insert(file_dep.path.clone(), file_hash.clone());
                file_hash
            }
        };
        current_task_input
            .file_hashes
            .insert(String::from(file_alias.as_ref()), current_hash);
    }

    for (task_alias, task_dep) in task.task_deps.iter() {
        let cached_task_output = cache.task_outputs.read().unwrap().get(task_dep).cloned();
        let current_task_output = match cached_task_output {
            Some(output) => output,
            None => {
                let task_record = get_task_record(&db_env, db.clone(), task_dep)
                    .map_err(|e| TaskExecutionError::DBGetError(e))?;
                cache
                    .task_outputs
                    .write()
                    .unwrap()
                    .insert(task_dep.clone(), task_record.output.task_output.clone());
                task_record.output.task_output
            }
        };
        current_task_input
            .task_outputs
            .insert(String::from(task_alias.as_ref()), current_task_output);
    }

    for (var_alias, var_name) in task.var_deps.iter() {
        let var_value = get_var(var_name.as_ref(), &workspace_config.vars)
            .map_err(|e| TaskExecutionError::VarLookupError(e))?;
        current_task_input
            .vars
            .insert(String::from(var_alias.as_ref()), var_value.clone());
    }

    Ok(current_task_input)
}

fn get_up_to_date_task_record(
    workspace_dir: &Path,
    db_env: &lmdb::Environment,
    db: &lmdb::Database,
    task: &TaskJob,
    current_task_input: &TaskInput,
) -> Option<TaskRecord> {
    let task_record_opt = match get_task_record(&db_env, db.clone(), task.task_name.as_ref()) {
        Ok(r) => Some(r),
        Err(e) => match e {
            GetError::NotFound(_) => None,
            _ => {
                panic!("Error retrieving task record from the database");
            }
        },
    };

    let task_record = match task_record_opt {
        Some(r) => r,
        None => {
            return None;
        }
    };

    // Check project source files
    if current_task_input.project_source_hashes.len()
        != task_record.input.project_source_hashes.len()
    {
        return None;
    }

    for (source_file, source_hash) in current_task_input.project_source_hashes.iter() {
        let prev_hash = match task_record.input.project_source_hashes.get(source_file) {
            Some(hash) => hash,
            None => {
                return None;
            }
        };

        if prev_hash != source_hash {
            return None;
        }
    }

    // Check input files
    if current_task_input.file_hashes.len() != task_record.input.file_hashes.len() {
        return None;
    }

    for (file_alias, hash) in current_task_input.file_hashes.iter() {
        let prev_hash = match task_record.input.file_hashes.get(file_alias) {
            Some(hash) => hash,
            None => {
                return None;
            }
        };

        if prev_hash != hash {
            return None;
        }
    }

    // Check outputs of task dependencies
    if current_task_input.task_outputs.len() != task_record.input.task_outputs.len() {
        return None;
    }

    for (task_alias, task_output) in current_task_input.task_outputs.iter() {
        let prev_task_output = match task_record.input.task_outputs.get(task_alias) {
            Some(output) => output,
            None => {
                return None;
            }
        };

        if prev_task_output != task_output {
            return None;
        }
    }

    // Check input variables
    if current_task_input.vars.len() != task_record.input.vars.len() {
        return None;
    }

    for (var_alias, var_value) in current_task_input.vars.iter() {
        let prev_var = match task_record.input.vars.get(var_alias) {
            Some(var) => var,
            None => {
                return None;
            }
        };

        if prev_var != var_value {
            return None;
        }
    }

    // Check output files
    let mut current_output_file_hashes: HashMap<Arc<str>, String> =
        HashMap::with_capacity(task.task.artifacts.len());
    for artifact in task.task.artifacts.iter() {
        let output_file_hash_res = compute_file_hash(
            workspace_dir
                .join(Path::new(artifact.filename.as_ref()))
                .as_path(),
        );
        match output_file_hash_res {
            Ok(hash) => {
                current_output_file_hashes.insert(artifact.filename.clone(), hash);
            }
            Err(_) => {
                return None;
            }
        };
    }

    if current_output_file_hashes.len() != task_record.output.file_hashes.len() {
        return None;
    }

    for (file_name, file_hash) in current_output_file_hashes {
        let prev_hash = match task_record.output.file_hashes.get(file_name.as_ref()) {
            Some(hash) => hash,
            None => {
                return None;
            }
        };

        if prev_hash != &file_hash {
            return None;
        }
    }

    Some(task_record)
}

fn execute_task_actions_and_store_result(
    workspace_dir: &Path,
    lua: &mlua::Lua,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    task: &TaskJob,
    task_result_sender: &Sender<TaskJobMessage>,
    stdin_ready: &Arc<(Mutex<bool>, Condvar)>,
    cache: &Arc<TaskExecutorCache>,
    current_task_input: TaskInput,
) -> Result<(), TaskExecutionError> {

    if task.task.is_interactive {
        let (ready_lock, ready_condvar) = stdin_ready.as_ref();
        let mut ready = ready_lock.lock().unwrap();
        while !*ready {
            ready = ready_condvar.wait(ready).unwrap();
        }
        lua.set_named_registry_value(COBBLE_JOB_INTERACTIVE_ENABLED, true)
            .map_err(|e| TaskExecutionError::LuaError(e))?;
    }

    let result = execute_task_actions(
        lua,
        task,
        &current_task_input,
        &task.workspace,
        db_env,
        db,
        cache,
        &task_result_sender,
    )?;

    lua.set_named_registry_value(COBBLE_JOB_INTERACTIVE_ENABLED, false)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let mut detached_result: DetachedLuaValue = lua
        .unpack(result)
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let mut artifact_file_hashes: HashMap<String, String> =
        HashMap::with_capacity(task.task.artifacts.len());
    for artifact in task.task.artifacts.iter() {
        let output_file_hash_res = compute_file_hash(
            workspace_dir
                .join(Path::new(artifact.filename.as_ref()))
                .as_path(),
        );
        match output_file_hash_res {
            Ok(hash) => {
                artifact_file_hashes.insert(String::from(artifact.filename.as_ref()), hash);
            }
            Err(e) => {
                return Err(TaskExecutionError::DBPutError(PutError::FileError(e)));
            }
        };
    }

    // Usually, if a task runs, we'd want other tasks that depend on it to also run unless the task output or an artifact
    // has changed.  If there's nothing to compoare against, we'll add a timestamp so that providing no output or artifacts
    // means tasks that depend on this one will always run if this task was run
    if let DetachedLuaValue::Nil = detached_result {
        if artifact_file_hashes.len() == 0 {
            let time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                .expect("Time since unix epoch should not be negative");
            detached_result = DetachedLuaValue::Integer(time.as_millis() as i64);
        }
    }
    
    let task_output_record = TaskOutput {
        task_output: detached_result.to_json(),
        file_hashes: artifact_file_hashes,
    };

    let task_record = TaskRecord {
        input: current_task_input,
        output: task_output_record,
    };
    put_task_record(db_env, db.clone(), task.task_name.as_ref(), &task_record)
        .map_err(|e| TaskExecutionError::DBPutError(e))?;
    cache
        .task_outputs
        .write()
        .unwrap()
        .insert(task.task_name.clone(), task_record.output.task_output);
    Ok(())
}

pub fn execute_task_job(
    workspace_config: &Arc<WorkspaceConfig>,
    lua: &mlua::Lua,
    db_env: &Arc<lmdb::Environment>,
    db: &lmdb::Database,
    task: &TaskJob,
    task_result_sender: &Sender<TaskJobMessage>,
    stdin_ready: &Arc<(Mutex<bool>, Condvar)>,
    cache: Arc<TaskExecutorCache>,
) {
    if cache
        .task_outputs
        .read()
        .unwrap()
        .contains_key(&task.task_name)
    {
        task_result_sender
            .send(TaskJobMessage::Complete {
                task: task.task_name.clone(),
                result: TaskResult::UpToDate,
            })
            .unwrap();
        return;
    }

    let current_task_input_res =
        get_current_task_input(workspace_config, &task.task, db_env, db, &cache);
    let current_task_input = match current_task_input_res {
        Ok(task_input) => task_input,
        Err(e) => {
            task_result_sender
                .send(TaskJobMessage::Complete {
                    task: task.task_name.clone(),
                    result: TaskResult::Error(e),
                })
                .unwrap();
            return;
        }
    };

    if !workspace_config.force_run_tasks && !task.task.always_run {
        let up_to_date_task_record = get_up_to_date_task_record(
            &workspace_config.workspace_dir,
            db_env,
            db,
            task,
            &current_task_input,
        );

        if let Some(task_record) = up_to_date_task_record {
            cache
                .task_outputs
                .write()
                .unwrap()
                .insert(task.task_name.clone(), task_record.output.task_output);
            task_result_sender
                .send(TaskJobMessage::Complete {
                    task: task.task_name.clone(),
                    result: TaskResult::UpToDate,
                })
                .unwrap();
            return;
        }
    }

    let result = execute_task_actions_and_store_result(
        &workspace_config.workspace_dir,
        lua,
        db_env,
        db,
        task,
        task_result_sender,
        stdin_ready,
        &cache,
        current_task_input,
    );
    match result {
        Ok(_) => {
            task_result_sender
                .send(TaskJobMessage::Complete {
                    task: task.task_name.clone(),
                    result: TaskResult::Success,
                })
                .unwrap();
        }
        Err(e) => {
            task_result_sender
                .send(TaskJobMessage::Complete {
                    task: task.task_name.clone(),
                    result: TaskResult::Error(e),
                })
                .unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{mpsc, RwLock};
    use std::time::Duration;

    use crate::db::new_db_env;
    use crate::execute::action::init_lua_for_task_executor;
    use crate::lua::{lua_env::create_lua_env, detached::dump_function};
    use crate::project_def::{Action, ActionCmd, ExternalTool};
    use crate::workspace::{Task, TaskType, Workspace};

    use super::*;

    #[test]
    fn test_tool_check_job() {
        let tmpdir = mktemp::Temp::new_dir().unwrap();

        let workspace_config = Arc::new(WorkspaceConfig {
            workspace_dir: PathBuf::from("."),
            root_projects: vec![String::from(".")],
            vars: HashMap::new(),
            force_run_tasks: false,
            num_threads: 1
        });
        let workspace_dir: Arc<Path> = PathBuf::from(".").into();
        let lua = create_lua_env(workspace_dir.as_ref()).unwrap();
        init_lua_for_task_executor(&lua).unwrap();

        let db_env = Arc::new(new_db_env(tmpdir.as_path().join(".cobble.db").as_path()).unwrap());
        let db = db_env.open_db(None).unwrap();
        let (tx, rx) = mpsc::channel::<TaskJobMessage>();

        let cache = Arc::new(TaskExecutorCache {
            project_source_hashes: RwLock::new(HashMap::new()),
            file_hashes: RwLock::new(HashMap::new()),
            task_outputs: RwLock::new(HashMap::new()),
        });

        let tool_func: mlua::Function = lua
            .load(r#"function (c) assert(c.args[1] == "Test!") end"#)
            .eval()
            .unwrap();

        let tool_name = Arc::<str>::from("print");
        let tool = Arc::new(ExternalTool {
            name: tool_name.clone(),
            install: None,
            check: None,
            action: Action {
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Func(dump_function(&lua, tool_func, &mut HashMap::new()).unwrap()),
            },
        });

        let test_task_name = Arc::<str>::from("test");

        let task = Arc::new(Task {
            name: test_task_name.clone(),
            task_type: TaskType::Task,
            dir: workspace_dir.clone(),
            project_name: Arc::<str>::from("/"),
            tools: vec![(tool_name.clone(), tool_name.clone())]
                .into_iter()
                .collect(),
            actions: vec![Action {
                tools: vec![(tool_name.clone(), tool_name.clone())]
                    .into_iter()
                    .collect(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Cmd(vec![Arc::<str>::from("Test!")], HashMap::new()),
            }],
            ..Default::default()
        });

        let workspace = Arc::new(Workspace {
            tasks: vec![(test_task_name.clone(), task.clone())]
                .into_iter()
                .collect(),
            build_envs: HashMap::new(),
            tools: vec![(tool_name.clone(), tool.clone())]
                .into_iter()
                .collect(),
            file_providers: HashMap::new(),
        });

        let task_job = TaskJob {
            task_name: test_task_name.clone(),
            workspace: workspace.clone(),
            task: task.clone(),
        };

        let stdin_ready = Arc::new((Mutex::new(true), Condvar::new()));

        execute_task_job(
            &workspace_config,
            &lua,
            &db_env,
            &db,
            &task_job,
            &tx,
            &stdin_ready,
            cache.clone(),
        );

        let job_result = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        match job_result {
            TaskJobMessage::Complete { task: tgt, result } => {
                assert_eq!(tgt.as_ref(), "test");
                match result {
                    TaskResult::Success => { /* pass */ }
                    res => panic!("Did not get a success message: {:?}", res),
                }
            }
            _ => panic!("Did not get a completion message"),
        };
    }
}
