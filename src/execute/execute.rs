// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::cmp::max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

use crate::config::{TaskOutputCondition, WorkspaceConfig};
use crate::db::{new_db_env, DeleteError, GetError, PutError};
use crate::execute::job_io::ConcurrentIO;
use crate::execute::worker::{run_task_executor_worker, TaskExecutorWorkerArgs};
use crate::project_def::ExternalTool;
use crate::vars::VarLookupError;
use crate::workspace::{BuildEnv, Task, TaskType, Workspace};

pub enum ExecutorJob {
    Task(TaskJob),
    Clean(CleanJob),
    ToolCheck(ToolCheckJob),
    EnvAction(EnvActionJob),
}

#[derive(Debug)]
pub struct TaskJob {
    // task_name is the job_id
    pub task_name: Arc<str>,
    pub task: Arc<Task>,
    pub workspace: Arc<Workspace>,
}

#[derive(Debug)]
pub struct CleanJob {
    pub job_id: Arc<str>,
    pub task: Arc<Task>,
    pub workspace: Arc<Workspace>,
}

#[derive(Debug)]
pub struct ToolCheckJob {
    pub job_id: Arc<str>,
    pub tool_name: Arc<str>,
    pub tool: Arc<ExternalTool>,
    pub workspace: Arc<Workspace>,
}

#[derive(Debug)]
pub struct EnvActionJob {
    pub job_id: Arc<str>,
    pub env: Arc<BuildEnv>,
    pub args: Vec<Arc<str>>,
    pub workspace: Arc<Workspace>,
}

#[derive(Debug)]
pub enum TaskJobMessage {
    Started {
        task: Arc<str>,
        stdin_ready: Arc<(Mutex<bool>, Condvar)>,
        show_stdout: TaskOutputCondition,
        show_stderr: TaskOutputCondition        
    },
    Stdout {
        task: Arc<str>,
        s: String,
    },
    Stderr {
        task: Arc<str>,
        s: String,
    },
    Complete {
        task: Arc<str>,
        result: TaskResult,
    },
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
    IOError { message: String, cause: io::Error },
    ExecutorError(String),
    DBGetError(GetError),
    DBPutError(PutError),
    DBDeleteError(DeleteError),
    LuaError(mlua::Error),
    ActionFailed(String),
    SerializeError(serde_json::Error),
    GraphError(String),
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
            IOError { message, cause } => write!(f, "{}: {}", message, cause),
            DBGetError(e) => write!(f, "Error reading value from the database: {}", e),
            DBPutError(e) => write!(f, "Error writing value to the database: {}", e),
            DBDeleteError(e) => write!(f, "Error deleting value from the database: {}", e),
            LuaError(e) => write!(f, "Lua error: {}", e),
            ActionFailed(s) => write!(f, "Action failed: {}", s),
            SerializeError(e) => write!(f, "(De)serialization of value failed: {}", e),
            GraphError(s) => write!(f, "{}", s),
        }
    }
}

fn get_tool_check_job_id(tool_name: &Arc<str>) -> Arc<str> {
    let mut job_name = String::from("tool_check:");
    job_name.push_str(tool_name.as_ref());
    Arc::<str>::from(job_name)
}

fn get_env_action_job_id(env_name: &Arc<str>) -> Arc<str> {
    let mut job_name = String::from("env_action:");
    job_name.push_str(env_name.as_ref());
    Arc::<str>::from(job_name)
}

fn get_task_job_dependencies(task: &Task, workspace: &Workspace) -> Result<Vec<Arc<str>>, TaskExecutionError> {
    let mut deps_set: HashSet<Arc<str>> = HashSet::with_capacity(task.task_deps.len());
    
    for task_dep in task.task_deps.values() {
        deps_set.insert(task_dep.clone());
    }

    for file_dep in task.file_deps.values() {
        if let Some(provided_by_task) = &file_dep.provided_by_task {
            deps_set.insert(provided_by_task.clone());
        }
    }

    for env_name in task.build_envs.values() {
        let env = workspace.build_envs.get(env_name)
            .ok_or_else(|| TaskExecutionError::EnvLookupError(env_name.clone()))?;

        if let Some(setup_task) = &env.setup_task {
            deps_set.insert(setup_task.clone());
        }
    }

    Ok(deps_set.into_iter().collect())
}

fn add_task_jobs(
    task_name: &Arc<str>,
    workspace: &Arc<Workspace>,
    jobs: &mut HashMap<Arc<str>, ExecutorJob>,
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

    let job = ExecutorJob::Task(TaskJob {
        task_name: task_name.to_owned(),
        task: task.clone(),
        workspace: workspace.clone(),
    });

    jobs.insert(task_name.to_owned(), job);

    for dep in get_task_job_dependencies(&*task, workspace.as_ref())? {
        add_task_jobs(&dep, workspace, jobs)?;
    }

    Ok(())
}

pub fn get_clean_task_name(task_name: &str) -> Arc<str> {
    let mut clean_task_name = String::from("clean:");
    clean_task_name.push_str(task_name);
    clean_task_name.into()
}

fn get_clean_job_task_dependencies(task: &Task) -> Vec<Arc<str>> {
    let mut deps: HashSet<Arc<str>> = HashSet::new();

    for (_env_alias, env_name) in &task.build_envs {
        deps.insert(env_name.clone());
    }

    for clean_action in &task.clean_actions {
        for (_env_alias, env_name) in &clean_action.build_envs {
            deps.insert(env_name.clone());
        }
    }

    deps.into_iter().collect()
}

fn add_clean_jobs(
    task_name: &Arc<str>,
    workspace: &Arc<Workspace>,
    jobs: &mut HashMap<Arc<str>, ExecutorJob>,
) -> Result<(), TaskExecutionError> {
    // TODO: Add option to also clean dependencies.  Otherwise, we don't automatically
    // traverse dependencies when selecting tasks to clean
    let task = workspace
        .tasks
        .get(task_name)
        .ok_or_else(|| TaskExecutionError::TaskLookupError(task_name.clone()))?;

    let job_id = get_clean_task_name(task_name.as_ref());
    let job = ExecutorJob::Clean(CleanJob {
        job_id: job_id.clone(),
        task: task.clone(),
        workspace: workspace.clone(),
    });

    jobs.insert(job_id, job);

    for dep in get_clean_job_task_dependencies(task.as_ref()) {
        add_task_jobs(&dep, workspace, jobs)?;
    }

    if let TaskType::Project = task.task_type {
        for dep in get_task_job_dependencies(task.as_ref(), workspace.as_ref())? {
            add_clean_jobs(&dep, workspace, jobs)?;
        }
    }

    Ok(())
}

fn add_tool_check_jobs(
    tool_name: &Arc<str>,
    workspace: &Arc<Workspace>,
    jobs: &mut HashMap<Arc<str>, ExecutorJob>,
) -> Result<(), TaskExecutionError> {
    let tool_check_job_id = get_tool_check_job_id(tool_name);
    if jobs.contains_key(&tool_check_job_id) {
        return Ok(());
    }

    let tool = workspace
        .tools
        .get(tool_name)
        .ok_or_else(|| TaskExecutionError::ToolLookupError(tool_name.clone()))?;

    let tool_check_job = ExecutorJob::ToolCheck(ToolCheckJob {
        job_id: tool_check_job_id.clone(),
        tool_name: tool_name.clone(),
        tool: tool.clone(),
        workspace: workspace.clone(),
    });

    jobs.insert(tool_check_job_id, tool_check_job);

    for tool_dep in tool.action.tools.values() {
        add_tool_check_jobs(tool_dep, workspace, jobs)?;
    }

    if let Some(tool_check) = tool.check.as_ref() {
        for tool_dep in tool_check.tools.values() {
            add_tool_check_jobs(tool_dep, workspace, jobs)?;
        }
    }

    Ok(())
}

fn add_env_action_jobs(
    env_name: &Arc<str>,
    args: &Vec<Arc<str>>,
    workspace: &Arc<Workspace>,
    jobs: &mut HashMap<Arc<str>, ExecutorJob>,
) -> Result<(), TaskExecutionError> {
    let env_action_job_id = get_env_action_job_id(env_name);

    let env = workspace.build_envs.get(env_name)
        .ok_or_else(|| TaskExecutionError::EnvLookupError(env_name.clone()))?;

    let env_action_job = EnvActionJob {
        job_id: env_action_job_id.clone(),
        env: env.clone(),
        args: args.clone(),
        workspace: workspace.clone()
    };

    jobs.insert(env_action_job_id, ExecutorJob::EnvAction(env_action_job));

    if let Some(setup_task) = &env.setup_task {
        add_task_jobs(setup_task, workspace, jobs)?;
    }

    Ok(())
}

fn compute_dependency_edges(
    jobs: &HashMap<Arc<str>, ExecutorJob>,
    workspace: &Workspace
) -> Result<HashMap<Arc<str>, Vec<Arc<str>>>, TaskExecutionError> {
    let mut dep_edges: HashMap<Arc<str>, Vec<Arc<str>>> = HashMap::new();

    for (id, job) in jobs {
        match job {
            ExecutorJob::Task(task_job) => {
                let task_deps = dep_edges.entry(id.clone()).or_default();
                for dep in get_task_job_dependencies(&task_job.task, workspace)? {
                    task_deps.push(dep);
                }
                // If an "execute_after" task is in the graph, add that as a dependency, too
                for after_job in task_job.task.execute_after.iter() {
                    if jobs.contains_key(after_job) {
                        task_deps.push(after_job.clone());
                    }
                }
            }
            ExecutorJob::Clean(clean_job) => {
                for dep in get_clean_job_task_dependencies(&clean_job.task) {
                    // Make sure that if we depend on a task, it doesn't get cleaned before we run
                    let clean_dep_task_name = get_clean_task_name(&dep);
                    if jobs.contains_key(&clean_dep_task_name) {
                        dep_edges
                            .entry(clean_dep_task_name)
                            .or_default()
                            .push(id.clone());
                    }

                    dep_edges.entry(id.clone()).or_default().push(dep);
                }

                // Clean tasks must be executed after all regular tasks
                let clean_deps = dep_edges.entry(id.clone()).or_default();
                for (other_id, other_job) in jobs {
                    if let ExecutorJob::Task(_) = other_job {
                        if !clean_deps.contains(&other_id) {
                            clean_deps.push(other_id.clone());
                        }
                    }
                }
            }
            ExecutorJob::ToolCheck(tool_check_job) => match &tool_check_job.tool.check {
                Some(check_action) => {
                    let deps_iter = check_action
                        .tools
                        .values()
                        .map(|t| get_tool_check_job_id(t));

                    let tool_deps = dep_edges.entry(id.clone()).or_default();
                    for dep in deps_iter {
                        tool_deps.push(dep);
                    }
                }
                None => { /* Nothing to do */ }
            },
            ExecutorJob::EnvAction(env_action_job) => {
                if let Some(setup_task) = &env_action_job.env.setup_task {
                    dep_edges.entry(id.clone()).or_default().push(setup_task.clone());
                }
            }
        };
    }

    Ok(dep_edges)
}

fn compute_reverse_dependency_edges(
    dep_edges: &HashMap<Arc<str>, Vec<Arc<str>>>,
) -> HashMap<Arc<str>, Vec<Arc<str>>> {
    let mut rev_dep_edges: HashMap<Arc<str>, Vec<Arc<str>>> = HashMap::new();

    for (job_id, job_deps) in dep_edges {
        for job_dep in job_deps {
            rev_dep_edges
                .entry(job_dep.clone())
                .or_default()
                .push(job_id.clone());
        }
    }

    rev_dep_edges
}

fn has_cycle(dep_edges: &HashMap<Arc<str>, Vec<Arc<str>>>) -> Option<Arc<str>> {
    let mut visited: HashSet<Arc<str>> = HashSet::new();
    let mut cleared: HashSet<Arc<str>> = HashSet::new();
    for node in dep_edges.keys() {
        let cyclic_node = find_cyclic_node(node, dep_edges, &mut visited, &mut cleared);
        if cyclic_node.is_some() {
            return cyclic_node;
        }
    }

    None
}

fn find_cyclic_node(
    node_id: &Arc<str>,
    dep_edges: &HashMap<Arc<str>, Vec<Arc<str>>>,
    visited: &mut HashSet<Arc<str>>,
    cleared: &mut HashSet<Arc<str>>,
) -> Option<Arc<str>> {
    if cleared.contains(node_id) {
        return None;
    }

    if visited.contains(node_id) {
        return Some(node_id.clone());
    }

    visited.insert(node_id.clone());

    if let Some(deps) = dep_edges.get(node_id) {
        for dep in deps {
            let cyclic_node = find_cyclic_node(dep, dep_edges, visited, cleared);
            if cyclic_node.is_some() {
                return cyclic_node;
            }
        }
    }

    visited.remove(node_id);
    cleared.insert(node_id.clone());
    None
}

fn has_missing_dependencies(
    nodes: &HashMap<Arc<str>, ExecutorJob>,
    dep_edges: &HashMap<Arc<str>, Vec<Arc<str>>>,
) -> Option<(Arc<str>, Arc<str>)> {
    for (job_id, job_deps) in dep_edges {
        for job_dep in job_deps {
            if !nodes.contains_key(job_dep) {
                return Some((job_id.clone(), job_dep.clone()));
            }
        }
    }

    None
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
    job_queue: Arc<(Mutex<Option<VecDeque<ExecutorJob>>>, Condvar)>,
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
            job_queue: Arc::new((Mutex::new(Some(VecDeque::new())), Condvar::new())),
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
        let des_num_worker_threads = max(1, self.workspace_config.num_threads as usize);
        if cur_num_worker_threads < des_num_worker_threads {
            for _ in cur_num_worker_threads..des_num_worker_threads {
                let worker_args = TaskExecutorWorkerArgs {
                    workspace_config: self.workspace_config.clone(),
                    db_env: self.db_env.clone(),
                    db: self.db.clone(),
                    task_queue: self.job_queue.clone(),
                    task_result_sender: self.message_channel.0.clone(),
                    cache: self.cache.clone(),
                };

                let worker_thread = thread::spawn(move || run_task_executor_worker(worker_args));

                self.worker_threads.push(worker_thread);
            }
        }
    }

    pub fn check_tools<'a, T>(
        &mut self,
        workspace: &Workspace,
        tools: T,
    ) -> Result<(), TaskExecutionError>
    where
        T: Iterator<Item = &'a Arc<str>>,
    {
        self.ensure_worker_threads();

        let frozen_workspace = Arc::new(workspace.clone());
        let mut jobs: HashMap<Arc<str>, ExecutorJob> = HashMap::new();

        for tool in tools {
            add_tool_check_jobs(tool, &frozen_workspace, &mut jobs)?;
        }

        self.execute_graph(jobs, &frozen_workspace)
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

        let frozen_workspace = Arc::new(workspace.clone());
        let mut jobs: HashMap<Arc<str>, ExecutorJob> = HashMap::new();

        for task in tasks {
            add_task_jobs(task, &frozen_workspace, &mut jobs)?;
        }

        self.execute_graph(jobs, &frozen_workspace)
    }

    pub fn clean_tasks<'a, T>(
        &mut self,
        workspace: &Workspace,
        tasks: T,
    ) -> Result<(), TaskExecutionError>
    where
        T: Iterator<Item = &'a Arc<str>>,
    {
        self.ensure_worker_threads();

        let frozen_workspace = Arc::new(workspace.clone());
        let mut jobs: HashMap<Arc<str>, ExecutorJob> = HashMap::new();

        for task in tasks {
            add_clean_jobs(task, &frozen_workspace, &mut jobs)?;
        }

        self.execute_graph(jobs, &frozen_workspace)
    }

    pub fn do_env_actions<'a, E>(&mut self, workspace: &Workspace, envs: E, args: &Vec<Arc<str>>) -> Result<(), TaskExecutionError>
    where E: Iterator<Item = &'a Arc<str>>
    {
        self.ensure_worker_threads();

        let frozen_workspace = Arc::new(workspace.clone());
        let mut jobs: HashMap<Arc<str>, ExecutorJob> = HashMap::new();

        for env in envs {
            add_env_action_jobs(env, args, &frozen_workspace, &mut jobs)?;
        }

        self.execute_graph(jobs, &frozen_workspace)
    }

    fn execute_graph(
        &mut self,
        nodes: HashMap<Arc<str>, ExecutorJob>,
        workspace: &Arc<Workspace>
    ) -> Result<(), TaskExecutionError> {
        let dep_edges = &compute_dependency_edges(&nodes, workspace.as_ref())?;

        if let Some(cyclic_node) = has_cycle(dep_edges) {
            return Err(TaskExecutionError::GraphError(format!(
                "Encountered cycle in dependency graph at {}",
                cyclic_node
            )));
        }
        if let Some((job_id, missing_dep)) = has_missing_dependencies(&nodes, dep_edges) {
            return Err(TaskExecutionError::GraphError(format!(
                "{} dependency {} was not found",
                job_id, missing_dep
            )));
        }

        let rev_dep_edges = compute_reverse_dependency_edges(dep_edges);

        let mut in_progress_jobs: HashSet<Arc<str>> = HashSet::new();
        let mut completed_jobs: HashSet<Arc<str>> = HashSet::new();
        let mut concurrent_io = ConcurrentIO::new();

        let mut remaining_jobs = nodes;

        let total_jobs = remaining_jobs.len();

        let mut jobs_without_dependencies: Vec<Arc<str>> = Vec::new();
        for job_id in remaining_jobs.keys() {
            let has_deps = match dep_edges.get(job_id) {
                Some(deps) => deps.len() > 0,
                None => false,
            };

            if !has_deps {
                jobs_without_dependencies.push(job_id.clone());
            }
        }

        for task_name in jobs_without_dependencies.iter() {
            let job = remaining_jobs.remove(task_name).expect(
                "indexing into HashMap using a key just read from the HashMap should not fail",
            );
            self.push_task_job(task_name, job, &mut in_progress_jobs)?;
        }

        while completed_jobs.len() < total_jobs {
            let message = self.message_channel.1.recv().map_err(|_| {
                TaskExecutionError::ExecutorError(String::from(
                    "Executor message channel closed before all tasks completed",
                ))
            })?;

            match message {
                TaskJobMessage::Started { task, stdin_ready, show_stdout, show_stderr } => {
                    concurrent_io.job_started(&task, stdin_ready, show_stdout, show_stderr);
                }
                TaskJobMessage::Stdout { task, s } => {
                    concurrent_io.print_stdout(&task, s);
                }
                TaskJobMessage::Stderr { task, s } => {
                    concurrent_io.print_stderr(&task, s);
                }
                TaskJobMessage::Complete { task, result } => {
                    completed_jobs.insert(task.clone());
                    in_progress_jobs.remove(task.as_ref());

                    concurrent_io.job_completed(&task, &result);

                    if let TaskResult::Error(e) = result {
                        return Err(e);
                    }

                    let node_rev_dep_edges_opt = rev_dep_edges.get(&task);
                    if let Some(node_rev_dep_edges) = node_rev_dep_edges_opt {
                        for fwd_job_id in node_rev_dep_edges.iter() {
                            let fwd_job_is_available = match remaining_jobs.get(fwd_job_id) {
                                Some(_fwd_job) => dep_edges
                                    .get(fwd_job_id)
                                    .unwrap()
                                    .iter()
                                    .all(|t_dep| completed_jobs.contains(t_dep)),
                                None => false,
                            };
                            if fwd_job_is_available {
                                let job = remaining_jobs.remove(fwd_job_id)
                                    .expect("indexing into HashMap using a key just read from the HashMap should not fail");
                                self.push_task_job(fwd_job_id, job, &mut in_progress_jobs)?;
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
        task_id: &Arc<str>,
        task_job: ExecutorJob,
        in_progress_jobs: &mut HashSet<Arc<str>>,
    ) -> Result<(), TaskExecutionError> {
        let (task_queue_mutex, task_queue_cvar) = &*self.job_queue;
        {
            in_progress_jobs.insert(task_id.clone());
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
            let (task_queue_mutex, task_queue_cvar) = &*self.job_queue;
            let mut task_queue = task_queue_mutex.lock().unwrap();
            *task_queue = None;

            task_queue_cvar.notify_all();
        }

        for worker in self.worker_threads.drain(..) {
            let _ = worker.join();
        }
    }
}
