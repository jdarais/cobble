use std::collections::{hash_map, HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

use crate::lua::lua_env::create_lua_env;
use crate::lua::serialized::SerializedLuaValue;
use crate::project_def::types::json_to_lua;
use crate::project_def::ExternalTool;
use crate::util::hash::compute_file_hash;
use crate::cobl::config::WorkspaceConfig;
use crate::cobl::db::{
    get_task_record, new_db_env, put_task_record, GetError, PutError, TaskInput, TaskOutput,
    TaskRecord,
};
use crate::cobl::workspace::{Task, Workspace};
use crate::cobl::vars::{get_var, VarLookupError};

#[derive(Debug)]
pub struct TaskJob {
    pub task_name: Arc<str>,
    pub task: Arc<Task>,
    pub workspace: Arc<Workspace>,
}

#[derive(Debug)]
pub enum TaskJobMessage {
    Stdout { task: Arc<str>, s: String },
    Stderr { task: Arc<str>, s: String },
    Complete { task: Arc<str>, result: TaskResult },
}

#[derive(Debug)]
pub enum TaskResult {
    Success,
    UpToDate,
    Error(TaskExecutionError),
}

#[derive(Debug)]
pub enum TaskExecutionError {
    TaskLookupError(Arc<str>),
    VarLookupError(VarLookupError),
    ToolLookupError(Arc<str>),
    EnvLookupError(Arc<str>),
    TaskResultError { task: Arc<str>, message: String },
    UnresolvedCalcDependencyError(Arc<str>),
    IOError(io::Error),
    ExecutorError(String),
    DBGetError(GetError),
    DBPutError(PutError),
    LuaError(mlua::Error),
    ActionFailed(String),
    SerializeError(serde_json::Error),
}

impl Error for TaskExecutionError {}
impl fmt::Display for TaskExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TaskExecutionError::*;
        match self {
            TaskLookupError(t) => write!(f, "Task not found while creating jobs: {}", t),
            VarLookupError(e) => write!(f, "Error retrieving variable: {}", e),
            ToolLookupError(t) => write!(f, "Tool not found while creating jobs: {}", t),
            EnvLookupError(env) => write!(f, "Build env not found while creating jbos: {}", env),
            TaskResultError { task, message } => write!(
                f,
                "Execution of task {} failed with error: {}",
                task, message
            ),
            UnresolvedCalcDependencyError(t) => write!(
                f,
                "Encountered a task with unresolved calc dependencies: {}",
                t
            ),
            ExecutorError(e) => write!(f, "Error running task executors: {}", e),
            IOError(e) => write!(f, "I/O Error: {}", e),
            DBGetError(e) => write!(f, "Error reading value from the database: {}", e),
            DBPutError(e) => write!(f, "Error writing value to the database: {}", e),
            LuaError(e) => write!(f, "Lua error: {}", e),
            ActionFailed(s) => write!(f, "Action failed: {}", s),
            SerializeError(e) => write!(f, "(De)serialization of value failed: {}", e),
        }
    }
}

pub fn strip_error_context(error: &mlua::Error) -> mlua::Error {
    match error {
        mlua::Error::WithContext { context: _, cause } => strip_error_context(&*cause),
        mlua::Error::CallbackError {
            traceback: _,
            cause,
        } => strip_error_context(&*cause),
        _ => error.clone(),
    }
}

fn get_task_job_dependencies<'a>(task: &'a Task) -> Vec<Arc<str>> {
    task.task_deps
        .values()
        .cloned()
        .chain(
            task.file_deps
                .values()
                .filter_map(|f| f.provided_by_task.iter().next().cloned()),
        )
        .collect()
}

fn compute_task_job_forward_edges(workspace: &Workspace) -> HashMap<Arc<str>, Vec<Arc<str>>> {
    let mut forward_edges: HashMap<Arc<str>, HashSet<Arc<str>>> = HashMap::new();

    for (task_name, task) in workspace.tasks.iter() {
        for task_dep in get_task_job_dependencies(task.as_ref()) {
            forward_edges
                .entry(task_dep.clone())
                .or_default()
                .insert(task_name.clone());
        }

        for execute_after in task.execute_after.iter() {
            forward_edges
                .entry(execute_after.clone())
                .or_default()
                .insert(task_name.clone());
        }
    }

    forward_edges
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect()
}

pub fn create_jobs_for_tasks<'a, T>(
    workspace: &Arc<Workspace>,
    tasks: T,
) -> Result<HashMap<Arc<str>, TaskJob>, TaskExecutionError>
where
    T: Iterator<Item = &'a Arc<str>>,
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
    jobs: &mut HashMap<Arc<str>, TaskJob>,
) -> Result<(), TaskExecutionError> {
    if jobs.contains_key(task_name) {
        return Ok(());
    }

    let task = workspace
        .tasks
        .get(task_name)
        .ok_or_else(|| TaskExecutionError::TaskLookupError(task_name.clone()))?;

    if task.calc_deps.len() > 0 {
        return Err(TaskExecutionError::UnresolvedCalcDependencyError(
            task_name.clone(),
        ));
    }

    let job = TaskJob {
        task_name: task_name.to_owned(),
        task: task.clone(),
        workspace: workspace.clone(),
    };

    jobs.insert(task_name.to_owned(), job);

    for dep in get_task_job_dependencies(&*task) {
        add_jobs_for_task(&dep, workspace, jobs)?;
    }

    Ok(())
}

pub struct TaskExecutorCache {
    pub project_source_hashes: RwLock<HashMap<Arc<str>, String>>,
    pub file_hashes: RwLock<HashMap<Arc<str>, String>>,
    pub task_outputs: RwLock<HashMap<Arc<str>, serde_json::Value>>,
}

pub enum TaskConsoleOutput {
    Out(String),
    Err(String),
}

pub struct TaskExecutor {
    worker_threads: Vec<JoinHandle<()>>,
    workspace_config: Arc<WorkspaceConfig>,
    db_env: Arc<lmdb::Environment>,
    db: lmdb::Database,
    task_queue: Arc<(Mutex<Option<VecDeque<TaskJob>>>, Condvar)>,
    message_channel: (Sender<TaskJobMessage>, Receiver<TaskJobMessage>),
    cache: Arc<TaskExecutorCache>,
}

impl TaskExecutor {
    pub fn new(config: Arc<WorkspaceConfig>, db_path: &Path) -> anyhow::Result<TaskExecutor> {
        let db_env = new_db_env(db_path)?;
        let db = db_env.open_db(None)?;
        Ok(TaskExecutor {
            worker_threads: Vec::new(),
            workspace_config: config,
            db_env: Arc::new(db_env),
            db: db,
            task_queue: Arc::new((Mutex::new(Some(VecDeque::new())), Condvar::new())),
            message_channel: mpsc::channel(),
            cache: Arc::new(TaskExecutorCache {
                project_source_hashes: RwLock::new(HashMap::new()),
                file_hashes: RwLock::new(HashMap::new()),
                task_outputs: RwLock::new(HashMap::new()),
            }),
        })
    }

    pub fn cache(&self) -> Arc<TaskExecutorCache> {
        self.cache.clone()
    }

    pub fn ensure_worker_threads(&mut self) {
        self.worker_threads = self
            .worker_threads
            .drain(..)
            .filter(|t| !t.is_finished())
            .collect();
        let cur_num_worker_threads = self.worker_threads.len();
        if cur_num_worker_threads < 5 {
            for _ in cur_num_worker_threads..5 {
                let worker_args = TaskExecutorWorkerArgs {
                    workspace_config: self.workspace_config.clone(),
                    db_env: self.db_env.clone(),
                    db: self.db.clone(),
                    task_queue: self.task_queue.clone(),
                    task_result_sender: self.message_channel.0.clone(),
                    cache: self.cache.clone(),
                };

                let worker_thread = thread::spawn(move || run_task_executor_worker(worker_args));

                self.worker_threads.push(worker_thread);
            }
        }
    }

    pub fn execute_tasks<'a, T>(
        &mut self,
        workspace: &Workspace,
        tasks: T,
    ) -> Result<(), TaskExecutionError>
    where
        T: Iterator<Item = &'a Arc<str>>,
    {
        self.ensure_worker_threads();

        let mut in_progress_jobs: HashSet<Arc<str>> = HashSet::new();
        let mut completed_jobs: HashSet<Arc<str>> = HashSet::new();
        let mut task_output_buffers: HashMap<Arc<str>, Vec<TaskConsoleOutput>> = HashMap::new();
        let mut current_output_task: Option<Arc<str>> = None;

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
            let job = remaining_jobs.remove(task_name).expect(
                "indexing into HashMap using a key just read from the HashMap should not fail",
            );
            self.push_task_job(job, &mut in_progress_jobs)?;
        }

        while completed_jobs.len() < total_jobs {
            let message = self.message_channel.1.recv().map_err(|_| {
                TaskExecutionError::ExecutorError(String::from(
                    "Executor message channel closed before all tasks completed",
                ))
            })?;

            match message {
                TaskJobMessage::Stdout { task, s } => {
                    let is_current_output_task = match current_output_task.as_ref() {
                        Some(cur_out_task) => &task == cur_out_task,
                        None => {
                            current_output_task = Some(task.clone());
                            true
                        }
                    };

                    if is_current_output_task {
                        print!("{}", s);
                    } else {
                        match task_output_buffers.entry(task.clone()) {
                            hash_map::Entry::Occupied(mut ent) => {
                                ent.get_mut().push(TaskConsoleOutput::Out(s));
                            }
                            hash_map::Entry::Vacant(ent) => {
                                ent.insert(vec![TaskConsoleOutput::Out(s)]);
                            }
                        }
                    }
                }
                TaskJobMessage::Stderr { task, s } => {
                    let is_current_output_task = match current_output_task.as_ref() {
                        Some(cur_out_task) => &task == cur_out_task,
                        None => {
                            current_output_task = Some(task.clone());
                            true
                        }
                    };

                    if is_current_output_task {
                        let _ = write!(io::stderr(), "{}", s);
                    } else {
                        match task_output_buffers.entry(task.clone()) {
                            hash_map::Entry::Occupied(mut ent) => {
                                ent.get_mut().push(TaskConsoleOutput::Err(s));
                            }
                            hash_map::Entry::Vacant(ent) => {
                                ent.insert(vec![TaskConsoleOutput::Err(s)]);
                            }
                        }
                    }
                }
                TaskJobMessage::Complete { task, result } => {
                    completed_jobs.insert(task.clone());
                    in_progress_jobs.remove(task.as_ref());
                    match result {
                        TaskResult::UpToDate => {
                            println!("{} is up to date", task);
                        }
                        TaskResult::Success => {
                            println!("{} succeeded", task);
                        }
                        TaskResult::Error(e) => {
                            return Err(e);
                        }
                    }

                    let is_current_output_task =
                        current_output_task.as_ref().map_or(false, |t| t == &task);
                    if is_current_output_task {
                        for completed_job in completed_jobs.iter() {
                            if let Some(buffered_output) = task_output_buffers.remove(completed_job)
                            {
                                for task_output in buffered_output {
                                    match task_output {
                                        TaskConsoleOutput::Out(s) => {
                                            print!("{}", s);
                                        }
                                        TaskConsoleOutput::Err(s) => {
                                            let _ = write!(io::stderr(), "{}", s);
                                        }
                                    }
                                }
                            }
                        }

                        current_output_task = task_output_buffers.keys().cloned().next();
                        if let Some(cur_out_task) = current_output_task.as_ref() {
                            let cur_out_buffer = task_output_buffers.remove(cur_out_task).expect(
                                "cur_out_task should always exist in the task_output_buffers list",
                            );
                            for task_output in cur_out_buffer {
                                match task_output {
                                    TaskConsoleOutput::Out(s) => {
                                        print!("{}", s);
                                    }
                                    TaskConsoleOutput::Err(s) => {
                                        let _ = write!(io::stderr(), "{}", s);
                                    }
                                }
                            }
                        }
                    }

                    let forward_edges_from_task = forward_edges.get(&task);
                    if let Some(fwd_edges) = forward_edges_from_task {
                        for fwd_edge in fwd_edges.iter() {
                            let fwd_job_is_available = match remaining_jobs.get(fwd_edge) {
                                Some(fwd_job) => {
                                    let deps_satisfied = fwd_job
                                        .task
                                        .task_deps
                                        .iter()
                                        .all(|(_, t_dep)| completed_jobs.contains(t_dep));
                                    let execute_after_satisfied =
                                        fwd_job.task.execute_after.iter().all(|ex_after| {
                                            !remaining_jobs.contains_key(ex_after)
                                                && !in_progress_jobs.contains(ex_after)
                                        });
                                    deps_satisfied && execute_after_satisfied
                                }
                                None => false,
                            };
                            if fwd_job_is_available {
                                let job = remaining_jobs.remove(fwd_edge)
                                    .expect("indexing into HashMap using a key just read from the HashMap should not fail");
                                self.push_task_job(job, &mut in_progress_jobs)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn push_task_job(
        &mut self,
        task_job: TaskJob,
        in_progress_jobs: &mut HashSet<Arc<str>>,
    ) -> Result<(), TaskExecutionError> {
        let (task_queue_mutex, task_queue_cvar) = &*self.task_queue;
        {
            in_progress_jobs.insert(task_job.task_name.clone());
            let mut task_queue_opt = task_queue_mutex.lock().unwrap();
            if let Some(task_queue) = task_queue_opt.as_mut() {
                task_queue.push_back(task_job);
            }
        }
        task_queue_cvar.notify_one();
        Ok(())
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
    let task_executor_env_source = include_bytes!("task_executor.lua");
    lua.load(&task_executor_env_source[..]).exec()
}

struct TaskExecutorWorkerArgs {
    pub workspace_config: Arc<WorkspaceConfig>,
    pub db_env: Arc<lmdb::Environment>,
    pub db: lmdb::Database,
    pub task_queue: Arc<(Mutex<Option<VecDeque<TaskJob>>>, Condvar)>,
    pub task_result_sender: Sender<TaskJobMessage>,
    pub cache: Arc<TaskExecutorCache>,
}

fn poll_next_task(task_queue: &(Mutex<Option<VecDeque<TaskJob>>>, Condvar)) -> Option<TaskJob> {
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

fn run_task_executor_worker(args: TaskExecutorWorkerArgs) {
    let lua = create_lua_env(args.workspace_config.workspace_dir.as_path())
        .expect("Lua environment creation should always succeed");
    init_lua_for_task_executor(&lua)
        .expect("Initializing lua environment for a task executor should always succeed");

    loop {
        let next_task_opt = poll_next_task(&args.task_queue);

        match next_task_opt {
            Some(next_task) => {
                execute_task_job(
                    &args.workspace_config,
                    &lua,
                    args.db_env.as_ref(),
                    &args.db,
                    &next_task,
                    args.task_result_sender.clone(),
                    args.cache.clone(),
                );
            }
            None => {
                return;
            }
        };
    }
}

fn ensure_tool_is_cached(
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

fn ensure_build_env_is_cached(
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

fn execute_task_actions<'lua>(
    lua: &'lua mlua::Lua,
    task: &TaskJob,
    task_inputs: &TaskInput,
    sender: &Sender<TaskJobMessage>,
) -> Result<mlua::Value<'lua>, TaskExecutionError> {
    // Make sure build envs and tools we need are
    for (_, t_name) in task.task.tools.iter() {
        ensure_tool_is_cached(lua, t_name.as_ref(), task.workspace.as_ref())
            .map_err(|e| TaskExecutionError::LuaError(e))?;
    }

    for (_, e_name) in task.task.build_envs.iter() {
        ensure_build_env_is_cached(lua, e_name.as_ref(), task.workspace.as_ref())
            .map_err(|e| TaskExecutionError::LuaError(e))?;
    }

    let extra_tools: HashMap<&str, &str> = task
        .task
        .tools
        .iter()
        .map(|(k, v)| (k.as_ref(), v.as_ref()))
        .collect();
    let extra_build_envs: HashMap<&str, &str> = task
        .task
        .build_envs
        .iter()
        .map(|(k, v)| (k.as_ref(), v.as_ref()))
        .collect();

    // let file_hashes = task_inputs.file_hashes.clone();
    let files = lua
        .create_table()
        .and_then(|tbl| {
            for (k, v) in task_inputs.file_hashes.iter() {
                let file_tbl = lua.create_table()?;
                file_tbl.set(
                    "path",
                    task.task
                        .file_deps
                        .get(k.as_str())
                        .map(|dep| dep.path.to_string()),
                )?;
                file_tbl.set("hash", v.clone())?;
                tbl.set(k.clone(), file_tbl)?;
            }
            Ok(tbl)
        })
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let vars = task_inputs.vars.clone();

    let task_outputs = lua
        .create_table()
        .and_then(|tbl| {
            for (k, v) in task_inputs.task_outputs.iter() {
                tbl.set(k.clone(), json_to_lua(lua, v.clone())?)?;
            }
            Ok(tbl)
        })
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let project_dir = task
        .task
        .dir
        .to_str()
        .ok_or_else(|| {
            mlua::Error::runtime(format!(
                "Unable to convert path to a UTF-8 string: {}",
                task.task.dir.display()
            ))
        })
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let out_task_name_clone = task.task_name.clone();
    let out_sender_clone = sender.clone();
    let out = lua
        .create_function(move |_lua, s: String| {
            out_sender_clone
                .send(TaskJobMessage::Stdout {
                    task: out_task_name_clone.clone(),
                    s,
                })
                .map_err(|e| {
                    mlua::Error::runtime(format!(
                        "Error sending output from executor thread: {}",
                        e
                    ))
                })
        })
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let err_task_name_clone = task.task_name.clone();
    let err_sender_clone = sender.clone();
    let err = lua
        .create_function(move |_lua, s: String| {
            err_sender_clone
                .send(TaskJobMessage::Stderr {
                    task: err_task_name_clone.clone(),
                    s,
                })
                .map_err(|e| {
                    mlua::Error::runtime(format!(
                        "Error sending output from executor thread: {}",
                        e
                    ))
                })
        })
        .map_err(|e| TaskExecutionError::LuaError(e))?;

    let mut args: mlua::Value = mlua::Value::Nil;
    for action in task.task.actions.iter() {
        let action_lua = lua
            .pack(action.clone())
            .map_err(|e| TaskExecutionError::LuaError(e))?;

        let cobble_table: mlua::Table = lua
            .globals()
            .get("cobble")
            .map_err(|e| TaskExecutionError::LuaError(e))?;

        let create_action_context: mlua::Function = cobble_table
            .get("create_action_context")
            .map_err(|e| TaskExecutionError::LuaError(e))?;

        let action_context: mlua::Table = create_action_context
            .call((
                action_lua.clone(),
                extra_tools.clone(),
                extra_build_envs.clone(),
                files.clone(),
                vars.clone(),
                task_outputs.clone(),
                project_dir.to_owned(),
                out.clone(),
                err.clone(),
                args.clone(),
            ))
            .map_err(|e| TaskExecutionError::LuaError(e))?;

        let invoke_action_chunk = lua.load(r#"
            local action, action_context = ...
            return xpcall(cobble.invoke_action, function (msg) return msg end, action, action_context)
        "#);

        let action_result: mlua::MultiValue = invoke_action_chunk
            .call((action_lua, action_context))
            .map_err(|e| TaskExecutionError::LuaError(e))?;

        let mut action_result_iter = action_result.into_iter();
        let success = action_result_iter.next().unwrap_or(mlua::Value::Nil);
        let result = action_result_iter.next().unwrap_or(mlua::Value::Nil);

        let success_bool: bool = lua
            .unpack(success)
            .map_err(|e| TaskExecutionError::LuaError(e))?;
        if success_bool {
            args = result;
        } else {
            let message = match result {
                mlua::Value::String(s) => {
                    s.to_str().unwrap_or("<error reading message>").to_owned()
                }
                mlua::Value::Error(e) => e.to_string(),
                _ => format!("{:?}", result),
            };
            return Err(TaskExecutionError::ActionFailed(message));
        }
    }

    Ok(args)
}

fn get_current_task_input(
    workspace_config: &WorkspaceConfig,
    task: &TaskJob,
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

    for project_source in task.task.project_source_deps.iter() {
        let cached_hash = cache
            .project_source_hashes
            .read()
            .unwrap()
            .get(project_source)
            .cloned();
        let current_hash = match cached_hash {
            Some(hash) => hash,
            None => {
                let file_hash = compute_file_hash(
                    &workspace_config
                        .workspace_dir
                        .join(Path::new(project_source.as_ref()))
                        .as_path(),
                )
                .map_err(|e| TaskExecutionError::IOError(e))?;
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

    for (file_alias, file_dep) in task.task.file_deps.iter() {
        let cached_hash = cache
            .file_hashes
            .read()
            .unwrap()
            .get(&file_dep.path)
            .cloned();
        let current_hash = match cached_hash {
            Some(hash) => hash,
            None => {
                let file_hash = compute_file_hash(
                    workspace_config
                        .workspace_dir
                        .join(Path::new(file_dep.path.as_ref()))
                        .as_path(),
                )
                .map_err(|e| TaskExecutionError::IOError(e))?;
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

    for (task_alias, task_dep) in task.task.task_deps.iter() {
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

    for (var_alias, var_name) in task.task.var_deps.iter() {
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
    db_env: &lmdb::Environment,
    db: &lmdb::Database,
    task: &TaskJob,
    task_result_sender: &Sender<TaskJobMessage>,
    cache: &Arc<TaskExecutorCache>,
    current_task_input: TaskInput,
) -> Result<(), TaskExecutionError> {
    let result = execute_task_actions(lua, task, &current_task_input, &task_result_sender)?;
    let detached_result: SerializedLuaValue = lua
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

fn execute_task_job(
    workspace_config: &Arc<WorkspaceConfig>,
    lua: &mlua::Lua,
    db_env: &lmdb::Environment,
    db: &lmdb::Database,
    task: &TaskJob,
    task_result_sender: Sender<TaskJobMessage>,
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

    let current_task_input_res = get_current_task_input(workspace_config, task, db_env, db, &cache);
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
        &task_result_sender,
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
    extern crate mktemp;

    use std::{collections::HashSet, path::PathBuf, sync::mpsc, time::Duration};

    use crate::{
        lua::serialized::dump_function,
        project_def::{Action, ActionCmd},
        cobl::workspace::TaskType,
    };

    use super::*;

    #[test]
    fn test_execution_worker() {
        let tmpdir = mktemp::Temp::new_dir().unwrap();

        let workspace_config = Arc::new(WorkspaceConfig {
            workspace_dir: PathBuf::from("."),
            root_projects: vec![String::from(".")],
            vars: HashMap::new(),
            force_run_tasks: false,
        });
        let workspace_dir: Arc<Path> = PathBuf::from(".").into();
        let lua = create_lua_env(workspace_dir.as_ref()).unwrap();
        init_lua_for_task_executor(&lua).unwrap();

        let db_env = new_db_env(tmpdir.as_path().join(".cobble.db").as_path()).unwrap();
        let db = db_env.open_db(None).unwrap();
        let (tx, rx) = mpsc::channel::<TaskJobMessage>();

        let cache = Arc::new(TaskExecutorCache {
            project_source_hashes: RwLock::new(HashMap::new()),
            file_hashes: RwLock::new(HashMap::new()),
            task_outputs: RwLock::new(HashMap::new()),
        });

        let print_func: mlua::Function = lua
            .load(r#"function (c) print("Hi!", table.unpack(c.args)) end"#)
            .eval()
            .unwrap();

        let print_tool_name = Arc::<str>::from("print");
        let print_tool = Arc::new(ExternalTool {
            name: print_tool_name.clone(),
            install: None,
            check: None,
            action: Action {
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Func(dump_function(print_func, &lua, &HashSet::new()).unwrap()),
            },
        });

        let task = Arc::new(Task {
            task_type: TaskType::Task,
            dir: workspace_dir.clone(),
            project_name: Arc::<str>::from("/"),
            tools: vec![(print_tool_name.clone(), print_tool_name.clone())]
                .into_iter()
                .collect(),
            actions: vec![Action {
                tools: vec![(print_tool_name.clone(), print_tool_name.clone())]
                    .into_iter()
                    .collect(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Cmd(vec![Arc::<str>::from("There!")]),
            }],
            ..Default::default()
        });

        let test_task_name = Arc::<str>::from("test");

        let workspace = Arc::new(Workspace {
            tasks: vec![(test_task_name.clone(), task.clone())]
                .into_iter()
                .collect(),
            build_envs: HashMap::new(),
            tools: vec![(print_tool_name.clone(), print_tool.clone())]
                .into_iter()
                .collect(),
            file_providers: HashMap::new(),
        });

        let task_job = TaskJob {
            task_name: test_task_name.clone(),
            workspace: workspace.clone(),
            task: task.clone(),
        };

        execute_task_job(
            &workspace_config,
            &lua,
            &db_env,
            &db,
            &task_job,
            tx.clone(),
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
