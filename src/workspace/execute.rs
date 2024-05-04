extern crate lmdb;
extern crate sha2;
extern crate serde_json;


use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

use sha2::{Digest, Sha256};

use crate::datamodel::ExternalTool;
use crate::workspace::graph::{Workspace, Task};
use crate::lua::detached_value::DetachedLuaValue;
use crate::lua::lua_env::create_lua_env;
use crate::workspace::db::{get_task_record, new_db_env, put_task_record, GetError, PutError, TaskInput, TaskRecord};

#[derive(Debug)]
pub struct TaskJob {
    pub task_name: Arc<str>,
    pub task: Arc<Task>,
    pub workspace: Arc<Workspace>
}

#[derive(Debug)]
pub enum TaskJobMessage {
    Stdout{task: Arc<str>, s: String},
    Stderr{task: Arc<str>, s: String},
    Complete{task: Arc<str>, result: TaskResult},
}

#[derive(Debug)]
pub enum TaskResult {
    Success,
    UpToDate,
    Error(String)
}

#[derive(Debug)]
pub enum TaskExecutionError {
    TaskLookupError(Arc<str>),
    ToolLookupError(Arc<str>),
    EnvLookupError(Arc<str>),
    TaskResultError{task: Arc<str>, message: String},
    UnresolvedCalcDependencyError(Arc<str>)
}

impl fmt::Display for TaskExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TaskExecutionError::*;
        match self {
            TaskLookupError(t) => write!(f, "Task not found while creating jobs: {}", t),
            ToolLookupError(t) => write!(f, "Tool not found while creating jobs: {}", t),
            EnvLookupError(env) => write!(f, "Build env not found while creating jbos: {}", env),
            TaskResultError{task, message} => write!(f, "Execution of task {} failed with error: {}", task, message),
            UnresolvedCalcDependencyError(t) => write!(f, "Encountered a task with unresolved calc dependencies: {}", t)
        }
    }
}

pub fn strip_error_context(error: &mlua::Error) -> mlua::Error {
    match error {
        mlua::Error::WithContext{context: _, cause} => strip_error_context(&*cause),
        mlua::Error::CallbackError{traceback: _, cause} => strip_error_context(&*cause),
        _ => error.clone()
    }
}

fn get_task_job_dependencies<'a>(task: &'a Task) -> Vec<Arc<str>> {
    task.task_deps.iter().cloned()
    .chain(task.file_deps.iter().filter_map(|f| f.provided_by_task.iter().next().cloned()))
    .collect()
}

fn compute_task_job_forward_edges<'a>(workspace: &'a Workspace) -> HashMap<Arc<str>, Vec<Arc<str>>> {
    let mut forward_edges: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();

    for (task_name, task) in workspace.tasks.iter() {
        for task_dep in get_task_job_dependencies(task.as_ref()) {
            match forward_edges.get_mut(&task_dep) {
                Some(task_dep_forward_edges) => { task_dep_forward_edges.insert(task_name.clone()); },
                None => {
                    let mut task_dep_forward_edges: HashSet<Arc<str>> = HashSet::new();
                    task_dep_forward_edges.insert(task_name.clone());
                    forward_edges.insert(task_dep, task_dep_forward_edges);
                }
            }
        }
    }

    forward_edges.into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect()
}

pub fn create_jobs_for_tasks<'a, T>(workspace: &Arc<Workspace>, tasks: T) -> Result<HashMap<Arc<str>, TaskJob>, TaskExecutionError>
    where T: Iterator<Item = &'a Arc<str>>
{
    let mut jobs: HashMap<Arc<str>, TaskJob> = HashMap::new();

    for task in tasks {
        add_jobs_for_task(task, workspace, &mut jobs)?;
    }
    
    Ok(jobs)
}

fn add_jobs_for_task(
    task_name: &Arc<str>,
    workspace: &Arc<Workspace>,
    jobs: &mut HashMap<Arc<str>, TaskJob>
) -> Result<(), TaskExecutionError> {
    if jobs.contains_key(task_name) {
        return Ok(())
    }

    let task = workspace.tasks.get(task_name).ok_or_else(|| TaskExecutionError::TaskLookupError(task_name.clone()))?;

    if task.calc_deps.len() > 0 {
        return Err(TaskExecutionError::UnresolvedCalcDependencyError(task_name.clone()));
    }

    let job = TaskJob {
        task_name: task_name.to_owned(),
        task: task.clone(),
        workspace: workspace.clone()
    };

    jobs.insert(task_name.to_owned(), job);


    for dep in get_task_job_dependencies(&*task) {
        add_jobs_for_task(&dep, workspace, jobs)?;
    }

    Ok(())
}

pub struct TaskExecutorCache {
    pub file_hashes: RwLock<HashMap<Arc<str>, Vec<u8>>>,
    pub task_outputs: RwLock<HashMap<Arc<str>, serde_json::Value>>
}

pub struct TaskExecutor {
    worker_threads: Vec<JoinHandle<()>>,
    workspace_dir: PathBuf,
    db_path: PathBuf,
    task_queue: Arc<(Mutex<Option<VecDeque<TaskJob>>>, Condvar)>,
    message_channel: (Sender<TaskJobMessage>, Receiver<TaskJobMessage>),
    cache: Arc<TaskExecutorCache>
}

impl TaskExecutor {
    pub fn new(workspace_dir: &Path, db_path: &Path) -> TaskExecutor {
        TaskExecutor {
            worker_threads: Vec::new(),
            workspace_dir: PathBuf::from(workspace_dir),
            db_path: PathBuf::from(db_path),
            task_queue: Arc::new((Mutex::new(Some(VecDeque::new())), Condvar::new())),
            message_channel: mpsc::channel(),
            cache: Arc::new(TaskExecutorCache {
                file_hashes: RwLock::new(HashMap::new()),
                task_outputs: RwLock::new(HashMap::new())
            })
        }
    }

    pub fn cache(&self) -> Arc<TaskExecutorCache> {
        self.cache.clone()
    }

    pub fn ensure_worker_threads(&mut self) {
        if self.worker_threads.len() == 0 {
            for _ in 0..5 {
                let worker_args = TaskExecutorWorkerArgs {
                    workspace_dir: self.workspace_dir.clone(),
                    db_path: self.db_path.clone(),
                    task_queue: self.task_queue.clone(),
                    task_result_sender: self.message_channel.0.clone(),
                    cache: self.cache.clone()
                };
    
                let worker_thread = thread::spawn(move || {
                    run_task_executor_worker(worker_args)
                });
    
                self.worker_threads.push(worker_thread);
            }
        }
    }

    pub fn execute_tasks<'a, T>(&mut self, workspace: &Workspace, tasks: T) -> Result<(), TaskExecutionError>
        where T: Iterator<Item = &'a Arc<str>>
    {
        self.ensure_worker_threads();

        let mut completed_jobs: HashSet<Arc<str>> = HashSet::new();

        let forward_edges = compute_task_job_forward_edges(workspace);

        let frozen_workspace = Arc::new(workspace.clone());
        let mut remaining_jobs = create_jobs_for_tasks(&frozen_workspace, tasks)?;

        let total_jobs = remaining_jobs.len();

        let mut jobs_without_dependencies: Vec<Arc<str>> = Vec::new();
        for (task_name, task_job) in remaining_jobs.iter() {
            if task_job.task.task_deps.len() == 0 {
                jobs_without_dependencies.push(task_name.clone());
            }
        }

        for task_name in jobs_without_dependencies.iter() {
            let job = remaining_jobs.remove(task_name).unwrap();
            self.push_task_job(job);
        }

        while completed_jobs.len() < total_jobs {
            let message = self.message_channel.1.recv().unwrap();

            match message {
                TaskJobMessage::Stdout{task, s} => { print!("{}: {}", task, s); },
                TaskJobMessage::Stderr{task, s} => { let _ = write!(io::stderr(), "{}: {}", task, s); }
                TaskJobMessage::Complete{task, result} => {
                    completed_jobs.insert(task.clone());
                    match result {
                        TaskResult::UpToDate => { println!("{} is up to date", task); },
                        TaskResult::Success => { println!("{} succeeded", task); },
                        TaskResult::Error(e) => { return Err(TaskExecutionError::TaskResultError { task: task.clone(), message: e }); }
                    }

                    let forward_edges_from_task = forward_edges.get(&task);
                    if let Some(fwd_edges) = forward_edges_from_task {
                        for fwd_edge in fwd_edges.iter() {
                            let fwd_job_is_available = match remaining_jobs.get(fwd_edge) {
                                Some(fwd_job) => fwd_job.task.task_deps.iter().all(|d| completed_jobs.contains(d)),
                                None => false
                            };
                            if fwd_job_is_available {
                                self.push_task_job(remaining_jobs.remove(fwd_edge).unwrap());
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn push_task_job(&mut self, task_job: TaskJob) {
        let (task_queue_mutex, task_queue_cvar) = &*self.task_queue;
        {
            let mut task_queue_opt = task_queue_mutex.lock().unwrap();
            if let Some(task_queue) = task_queue_opt.as_mut() {
                task_queue.push_back(task_job);
            }
        }
        task_queue_cvar.notify_one();
    }
}

impl Drop for TaskExecutor {
    fn drop(&mut self) {
        {
            let (task_queue_mutex, task_queue_cvar) = &*self.task_queue;
            let mut task_queue = task_queue_mutex.lock().unwrap();
            *task_queue = None;

            task_queue_cvar.notify_all();
        }

        for worker in self.worker_threads.drain(..) {
            let _ = worker.join();
        }
    }
}

fn init_lua_for_task_executor(lua: &mlua::Lua) -> mlua::Result<()> {
    lua.load(r#"
        cobble = {
            _tool_cache = {},
            _build_env_cache = {}
        }

        local create_action_context, invoke_tool, invoke_build_env, invoke_action

        create_action_context = function (action, extra_tools, extra_build_envs, project_dir, out, err, args)
            local action_context = {
                tool = {},
                env = {},
                args = args,
                action = action,
                project = { dir = project_dir },
                out = out,
                err = err
            }

            for tool_alias, tool_name in pairs(extra_tools) do
                action_context.tool[tool_alias] = function (args)
                    return cobble.invoke_tool(tool_name, project_dir, out, err, args)
                end
            end
            for tool_alias, tool_name in pairs(action.tool) do
                action_context.tool[tool_alias] = function (args)
                    return cobble.invoke_tool(tool_name, project_dir, out, err, args)
                end
            end

            for env_alias, env_name in pairs(extra_build_envs) do
                action_context.env[env_alias] = function (args)
                    return cobble.invoke_build_env(env_name, project_dir, out, err, args)
                end
            end
            for env_alias, env_name in pairs(action.build_env) do
                action_context.env[env_alias] = function (args)
                    return cobble.invoke_build_env(env_name, project_dir, out, err, args)
                end
            end

            return action_context
        end

        invoke_action = function(action, action_context)
            if type(action[1]) == "function" then
                return action[1](action_context)
            else
                local tool_alias = next(action.tool)
                local env_alias = next(action.build_env)
                if tool_alias then
                    return action_context.tool[tool_alias](table.move(action, 1, #action, 1, {}))
                elseif env_alias then
                    return action_context.env[env_alias](table.move(action, 1, #action, 1, {}))
                else
                    return action_context.tool["cmd"](table.move(action, 1, #action, 1, {}))
                end
            end   
        end

        invoke_tool = function (name, project_dir, out, err, args)
            local action = cobble._tool_cache[name].action
            local action_context = create_action_context(action, {}, {}, project_dir, out, err, args)
            return invoke_action(action, action_context)
        end

        invoke_build_env = function (name, project_dir, out, err, args)
            local action = cobble._build_env_cache[name].action
            local action_context = create_action_context(action, {}, {}, project_dir, out, err, args)
            return invoke_action(action, action_context)
        end
        
        cobble.invoke_tool = invoke_tool
        cobble.invoke_build_env = invoke_build_env
        cobble.create_action_context = create_action_context
        cobble.invoke_action = invoke_action
    "#).exec()
}

struct TaskExecutorWorkerArgs {
    pub workspace_dir: PathBuf,
    pub db_path: PathBuf,
    pub task_queue: Arc<(Mutex<Option<VecDeque<TaskJob>>>, Condvar)>,
    pub task_result_sender: Sender<TaskJobMessage>,
    pub cache: Arc<TaskExecutorCache>
}

fn run_task_executor_worker(args: TaskExecutorWorkerArgs) {
    let lua = create_lua_env(args.workspace_dir.as_path()).unwrap();
    init_lua_for_task_executor(&lua).unwrap();

    let db_env = new_db_env(args.db_path.as_path()).unwrap();

    loop {
        let (task_queue_mutex, task_queue_cvar) = &*args.task_queue;
        let mut task_queue_locked = task_queue_mutex.lock().unwrap();
        loop {
            let task_available = match &*task_queue_locked {
                Some(queue) => !queue.is_empty(),
                None => { return; }
            };

            if task_available { break; }
    
            task_queue_locked = task_queue_cvar.wait(task_queue_locked).unwrap();
        }

        let next_task = task_queue_locked.as_mut().unwrap().pop_front().unwrap();
        drop(task_queue_locked);

        execute_task_job(args.workspace_dir.as_path(), &lua, &db_env, &next_task, args.task_result_sender.clone(), args.cache.clone());
    }
}

fn compute_file_hash(file_path: &Path) -> Result<Vec<u8>, io::Error> {
    let mut file_content: Vec<u8> = Vec::with_capacity(1024);
    let mut file = File::open(file_path)?;
    file.read_to_end(&mut file_content)?;

    let mut hasher = Sha256::new();
    hasher.update(&file_content);
    Ok(hasher.finalize().to_vec())
}

fn ensure_tool_is_cached(lua: &mlua::Lua, tool_name: &str, workspace: &Workspace) -> mlua::Result<()> {
    let tool = workspace.tools.get(tool_name)
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

fn ensure_build_env_is_cached(lua: &mlua::Lua, build_env_name: &str, workspace: &Workspace) -> mlua::Result<()> {
    let build_env = workspace.build_envs.get(build_env_name)
        .ok_or_else(|| mlua::Error::runtime(format!("Build env lookup failed: {}", build_env_name)))?;

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

#[derive(Debug)]
enum ExecuteTaskActionError {
    LuaError(mlua::Error),
    ActionFailed(String),
    SerializeError(serde_json::Error),
    SaveOutputError(PutError)
}

impl fmt::Display for ExecuteTaskActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecuteTaskActionError::*;
        match self {
            LuaError(e) => write!(f, "Lua error: {}", e),
            ActionFailed(s) => write!(f, "Action failed: {}", s),
            SerializeError(e) => write!(f, "(De)serialization of value failed: {}", e),
            SaveOutputError(e) => write!(f, "Error saving task output to database: {}", e)
        }
    }
}

fn execute_task_actions<'lua>(lua: &'lua mlua::Lua, task: &TaskJob, sender: &Sender<TaskJobMessage>) -> Result<mlua::Value<'lua>, ExecuteTaskActionError> {
    // Make sure build envs and tools we need are 
    for (_, t_name) in task.task.tools.iter() {
        ensure_tool_is_cached(lua, t_name.as_ref(), task.workspace.as_ref()).map_err(|e| ExecuteTaskActionError::LuaError(e))?;
    }

    for (_, e_name) in task.task.build_envs.iter() {
        ensure_build_env_is_cached(lua, e_name.as_ref(), task.workspace.as_ref()).map_err(|e| ExecuteTaskActionError::LuaError(e))?;
    }

    let extra_tools: HashMap<&str, &str> = task.task.tools.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();
    let extra_build_envs: HashMap<&str, &str> = task.task.build_envs.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();

    let project_dir = task.task.dir.to_str()
        .ok_or_else(|| mlua::Error::runtime(format!("Unable to convert path to a UTF-8 string: {}", task.task.dir.display())))
        .map_err(|e| ExecuteTaskActionError::LuaError(e))?;

    let out_task_name_clone = task.task_name.clone();
    let out_sender_clone = sender.clone();
    let out = lua.create_function(move |_lua, s: String| {
        out_sender_clone.send(TaskJobMessage::Stdout{task: out_task_name_clone.clone(), s})
            .map_err(|e| mlua::Error::runtime(format!("Error sending output from executor thread: {}", e)))
    }).map_err(|e| ExecuteTaskActionError::LuaError(e))?;

    let err_task_name_clone = task.task_name.clone();
    let err_sender_clone = sender.clone();
    let err = lua.create_function(move |_lua, s: String| {
        err_sender_clone.send(TaskJobMessage::Stderr{task: err_task_name_clone.clone(), s})
            .map_err(|e| mlua::Error::runtime(format!("Error sending output from executor thread: {}", e)))
    }).map_err(|e| ExecuteTaskActionError::LuaError(e))?;

    let mut args: mlua::Value = mlua::Value::Nil;
    for action in task.task.actions.iter() {
        let action_lua = lua.pack(action.clone())
            .map_err(|e| ExecuteTaskActionError::LuaError(e))?;

        let cobble_table: mlua::Table = lua.globals().get("cobble")
            .map_err(|e| ExecuteTaskActionError::LuaError(e))?;

        let create_action_context: mlua::Function = cobble_table.get("create_action_context")
            .map_err(|e| ExecuteTaskActionError::LuaError(e))?;

        let action_context: mlua::Table = create_action_context.call((
            action_lua.clone(),
            extra_tools.clone(),
            extra_build_envs.clone(),
            project_dir.to_owned(),
            out.clone(),
            err.clone(),
            args.clone())
        ).map_err(|e| ExecuteTaskActionError::LuaError(e))?;

        let invoke_action_chunk = lua.load(r#"
            local action, action_context = ...
            return xpcall(cobble.invoke_action, function (msg) return msg end, action, action_context)
        "#);

        let action_result: mlua::MultiValue = invoke_action_chunk.call((action_lua, action_context))
            .map_err(|e| ExecuteTaskActionError::LuaError(e))?;

        let mut action_result_iter = action_result.into_iter();
        let success = action_result_iter.next().unwrap_or(mlua::Value::Nil);
        let result = action_result_iter.next().unwrap_or(mlua::Value::Nil);

        let success_bool: bool = lua.unpack(success).map_err(|e| ExecuteTaskActionError::LuaError(e))?;
        if success_bool {
            args = result;
        } else {
            let message = match result {
                mlua::Value::String(s) => s.to_str().unwrap_or("<error reading message>").to_owned(),
                mlua::Value::Error(e) => e.to_string(),
                _ => format!("{:?}", result)
            };
            return Err(ExecuteTaskActionError::ActionFailed(message));
        }
    }

    Ok(args)
}

fn get_current_task_input(workspace_dir: &Path, task: &TaskJob, db_env: &lmdb::Environment, cache: &Arc<TaskExecutorCache>) -> TaskInput {
    let mut current_task_input = TaskInput {
        file_hashes: HashMap::new(),
        task_outputs: HashMap::new()
    };

    for file_dep in task.task.file_deps.iter() {
        let cached_hash = cache.file_hashes.read().unwrap().get(&file_dep.path).cloned();
        let current_hash = match cached_hash {
            Some(hash) => hash,
            None => {
                let file_hash = compute_file_hash(workspace_dir.join(Path::new(file_dep.path.as_ref())).as_path()).unwrap();
                cache.file_hashes.write().unwrap().insert(file_dep.path.clone(), file_hash.clone());
                file_hash
            }
        };
        current_task_input.file_hashes.insert(String::from(file_dep.path.as_ref()), current_hash);
    }

    for task_dep in task.task.task_deps.iter() {
        let cached_task_output = cache.task_outputs.read().unwrap().get(task_dep).cloned();
        let current_task_output = match cached_task_output {
            Some(output) => output,
            None => {
                let task_record = get_task_record(&db_env, task_dep).unwrap();
                cache.task_outputs.write().unwrap().insert(task_dep.clone(), task_record.output.clone());
                task_record.output
            }
        };
        current_task_input.task_outputs.insert(String::from(task_dep.as_ref()), current_task_output);
    }

    current_task_input
}

fn get_up_to_date_task_record(db_env: &lmdb::Environment, task: &TaskJob, current_task_input: &TaskInput) -> Option<TaskRecord> {
    if task.task.file_deps.len() == 0 && task.task.task_deps.len() == 0 {
        // If a task has no dependencies at all, there's nothing to check against to determine if the task is
        // up-to-date.  In this case, we assume that the author of the task intended for it to always be run
        return None;
    }

    let task_record_opt = match get_task_record(&db_env, task.task_name.as_ref()) {
        Ok(r) => Some(r),
        Err(e) => match e {
            GetError::NotFound(_) => None,
            _ => { panic!("Error retrieving task record from the database"); }
        }
    };

    let task_record = match task_record_opt {
        Some(r) => r,
        None => { return None; }
    };
    if current_task_input.file_hashes.len() != task_record.input.file_hashes.len() {
        return None;
    }

    for (path, hash) in current_task_input.file_hashes.iter() {
        let prev_hash = match task_record.input.file_hashes.get(path) {
            Some(hash) => hash,
            None => { return None; }
        };

        if prev_hash != hash {
            return None;
        }
    }

    if current_task_input.task_outputs.len() != task_record.input.task_outputs.len() {
        return None;
    }

    for (task_name, task_output) in current_task_input.task_outputs.iter() {
        let prev_task_output = match task_record.input.task_outputs.get(task_name) {
            Some(output) => output,
            None => { return None; }
        };

        if prev_task_output != task_output {
            return None;
        }
    }

    Some(task_record)
}

fn execute_task_actions_and_store_result(
    lua: &mlua::Lua,
    db_env: &lmdb::Environment,
    task: &TaskJob,
    task_result_sender: &Sender<TaskJobMessage>,
    cache: &Arc<TaskExecutorCache>,
    current_task_input: TaskInput
) -> Result<(), ExecuteTaskActionError> {
    let result = execute_task_actions(lua, task, &task_result_sender)?;
    let detached_result: DetachedLuaValue = lua.unpack(result).map_err(|e| ExecuteTaskActionError::LuaError(e))?;
    let task_record = TaskRecord { input: current_task_input, output: detached_result.to_json()};
    put_task_record(db_env, task.task_name.as_ref(), &task_record)
        .map_err(|e| ExecuteTaskActionError::SaveOutputError(e))?;
    cache.task_outputs.write().unwrap().insert(task.task_name.clone(), task_record.output);
    Ok(())
}

fn execute_task_job(workspace_dir: &Path, lua: &mlua::Lua, db_env: &lmdb::Environment, task: &TaskJob, task_result_sender: Sender<TaskJobMessage>, cache: Arc<TaskExecutorCache>) {
    let current_task_input = get_current_task_input(workspace_dir, task, db_env, &cache);
    let up_to_date_task_record = get_up_to_date_task_record(db_env, task, &current_task_input);

    if let Some(task_record) = up_to_date_task_record {
        cache.task_outputs.write().unwrap().insert(task.task_name.clone(), task_record.output);
        task_result_sender.send(TaskJobMessage::Complete {
            task: task.task_name.clone(),
            result: TaskResult::UpToDate
        }).unwrap();
        return;
    }

    let result = execute_task_actions_and_store_result(lua, db_env, task, &task_result_sender, &cache, current_task_input);
    
    match result {
        Ok(_) => {
            task_result_sender.send(TaskJobMessage::Complete {
                task: task.task_name.clone(),
                result: TaskResult::Success
            }).unwrap();
        },
        Err(e) => {
            let message = match e {
                ExecuteTaskActionError::ActionFailed(msg) => msg,
                ExecuteTaskActionError::LuaError(e) => e.to_string(),
                ExecuteTaskActionError::SerializeError(e) => e.to_string(),
                ExecuteTaskActionError::SaveOutputError(e) => e.to_string(),
            };

            task_result_sender.send(TaskJobMessage::Complete {
                task: task.task_name.clone(),
                result: TaskResult::Error(message)
            }).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate mktemp;

    use std::{collections::HashSet, sync::mpsc, time::Duration};

    use crate::{datamodel::{Action, ActionCmd}, lua::detached_value::dump_function, workspace::graph::TaskType};

    use super::*;

    #[test]
    fn test_execution_worker() {
        let tmpdir = mktemp::Temp::new_dir().unwrap();
        let workspace_dir: Arc<Path> = PathBuf::from(".").into();
        let lua = create_lua_env(workspace_dir.as_ref()).unwrap();
        init_lua_for_task_executor(&lua).unwrap();
    
        let db_env = new_db_env(tmpdir.as_path().join(".cobble.db").as_path()).unwrap();
        let (tx, rx) = mpsc::channel::<TaskJobMessage>();

        let cache = Arc::new(TaskExecutorCache {
            file_hashes: RwLock::new(HashMap::new()),
            task_outputs: RwLock::new(HashMap::new())
        });

        let print_func: mlua::Function = lua.load(r#"function (c) print("Hi!", table.unpack(c.args)) end"#).eval().unwrap();

        let print_tool_name = Arc::<str>::from("print");
        let print_tool = Arc::new(ExternalTool {
            name: print_tool_name.clone(),
            install: None,
            check: None,
            action: Action {
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Func(dump_function(print_func, &lua, &HashSet::new()).unwrap())
            }
        });

        let task = Arc::new(Task {
            task_type: TaskType::Task,
            dir: workspace_dir.clone(),
            project_name: Arc::<str>::from("/"),
            build_envs: HashMap::new(),
            tools: vec![(print_tool_name.clone(), print_tool_name.clone())].into_iter().collect(),
            file_deps: Vec::new(),
            task_deps: Vec::new(),
            calc_deps: Vec::new(),
            actions: vec![Action {
                tools: vec![(print_tool_name.clone(), print_tool_name.clone())].into_iter().collect(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Cmd(vec![Arc::<str>::from("There!")])
            }],
            artifacts: Vec::new()
        });

        let test_task_name = Arc::<str>::from("test");

        let workspace = Arc::new(Workspace {
            tasks: vec![(test_task_name.clone(), task.clone())].into_iter().collect(),
            build_envs: HashMap::new(),
            tools: vec![(print_tool_name.clone(), print_tool.clone())].into_iter().collect()
        });

        let task_job = TaskJob {
            task_name: test_task_name.clone(),
            workspace: workspace.clone(),
            task: task.clone()
        };

        execute_task_job(workspace_dir.as_ref(), &lua, &db_env, &task_job, tx.clone(), cache.clone());

        let job_result = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        match job_result {
            TaskJobMessage::Complete{task: tgt, result} => {
                assert_eq!(tgt.as_ref(), "test");
                match result {
                    TaskResult::Success => { /* pass */ },
                    _ => panic!("Did not get a success message")
                }
            },
            _ => panic!("Did not get a completion message")
        };
    }
}

