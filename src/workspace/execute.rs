extern crate lmdb;
extern crate sha2;
extern crate serde_json;


use std::{collections::{HashMap, VecDeque}, fmt, fs::File, io::{self, Read}, path::{Path, PathBuf}, sync::{mpsc::Sender, Arc, Condvar, Mutex, RwLock}, thread::Thread};

use lmdb::Transaction;
use sha2::{Digest, Sha256};

use crate::{datamodel::{Action, BuildEnv, ExternalTool, Task}, lua::{detached_value::DetachedLuaValue, lua_env::create_lua_env}, workspace::{db::{get_target_record, GetError}, query::{Workspace, WorkspaceTarget}}};

pub struct TargetJob {
    pub target_name: String,
    pub tools: HashMap<String, Arc<ExternalTool>>,
    pub envs: HashMap<String, Arc<BuildEnv>>,
    pub target: Arc<WorkspaceTarget>
}

#[derive(Debug)]
pub struct TargetJobMessage {
    pub target: String,
    pub message: TargetMessage
}

#[derive(Debug)]
pub enum TargetMessage {
    Output(String),
    Complete(TargetResult),
}

#[derive(Debug)]
pub enum TargetResult {
    Success,
    UpToDate,
    Error(String)
}

pub struct TargetStatus {
    pub up_to_date: bool
}

#[derive(Debug)]
pub enum CreateJobsError {
    TargetLookupError(String),
    ToolLookupError(String),
    EnvLookupError(String)
}

impl fmt::Display for CreateJobsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CreateJobsError::*;
        match self {
            TargetLookupError(t) => write!(f, "Target not found while creating jobs: {}", t),
            ToolLookupError(t) => write!(f, "Tool not found while creating jobs: {}", t),
            EnvLookupError(env) => write!(f, "Build env not found while creating jbos: {}", env)
        }
    }
}

pub fn create_jobs_for_targets<'a, T>(workspace: &Workspace, targets: T) -> Result<HashMap<String, TargetJob>, CreateJobsError>
    where T: Iterator<Item = &'a str>
{
    let mut jobs: HashMap<String, TargetJob> = HashMap::new();

    for target in targets {
        add_jobs_for_target(workspace, target, &mut jobs)?;
    }
    
    Ok(jobs)
}

fn add_jobs_for_target(workspace: &Workspace, target_name: &str, jobs: &mut HashMap<String, TargetJob>) -> Result<(), CreateJobsError> {
    if jobs.contains_key(target_name) {
        return Ok(())
    }

    let target = workspace.targets.get(target_name).ok_or_else(|| CreateJobsError::TargetLookupError(target_name.to_owned()))?;

    let mut job = TargetJob {
        target_name: target_name.to_owned(),
        tools: HashMap::new(),
        envs: HashMap::new(),
        target: target.clone()
    };

    for (t_alias, t_name) in target.tools.iter() {
        let tool = workspace.tools.get(t_name).ok_or_else(|| CreateJobsError::ToolLookupError(t_name.to_owned()))?;
        job.tools.insert(t_alias.clone(), tool.clone());
    }

    for (env_alias, env_name) in target.build_envs.iter() {
        let env = workspace.build_envs.get(env_name).ok_or_else(|| CreateJobsError::EnvLookupError(env_name.to_owned()))?;
        job.envs.insert(env_alias.clone(), env.clone());
    }

    jobs.insert(target_name.to_owned(), job);

    for dep in target.target_deps.iter() {
        add_jobs_for_target(workspace, dep.as_str(), jobs)?;
    }

    Ok(())
}

struct TaskExecutorCache {
    file_hashes: RwLock<HashMap<String, Vec<u8>>>,
    target_outputs: RwLock<HashMap<String, serde_json::Value>>
}

pub struct TaskExecutor<'db> {
    worker_threads: Vec<Thread>,
    db: &'db lmdb::Environment
}

impl <'db> TaskExecutor<'db> {
    pub fn new<'a>(db: &'a lmdb::Environment) -> TaskExecutor<'a> {
        TaskExecutor {
            worker_threads: Vec::new(),
            db
        }
    }


}

// struct TaskExecutorWorker {
//     lua: mlua::Lua,
//     task_queue: Arc<(Mutex<Option<VecDeque<TargetJob>>>, Condvar)>,
//     task_result_sender: Sender<TargetJobMessage>,
// }

fn init_lua_for_task_executor(lua: &mlua::Lua) -> mlua::Result<()> {
    lua.load(r#"
        cobble = {
            _tool_cache = {},
            _build_env_cache = {}
        }

        local create_action_context, invoke_tool, invoke_build_env, invoke_action

        create_action_context = function (action, extra_tools, extra_build_envs, project_dir, args)
            local action_context = {
                tool = {},
                env = {},
                args = args,
                action = action,
                project = { dir = project_dir }
            }

            for tool_alias, tool_name in pairs(extra_tools) do
                action_context.tool[tool_alias] = function (...)
                    return cobble.invoke_tool(tool_name, extra_tools, extra_build_envs, project_dir, table.pack(...))
                end
            end
            for tool_alias, tool_name in pairs(action.tool) do
                action_context.tool[tool_alias] = function (...)
                    return cobble.invoke_tool(tool_name, extra_tools, extra_build_envs, project_dir, table.pack(...))
                end
            end

            for env_alias, env_name in pairs(extra_build_envs) do
                action_context.env[env_alias] = function (...)
                    return cobble.invoke_build_env(env_name, extra_tools, extra_biuld_envs, project_dir, table.pack(...))
                end
            end
            for env_alias, env_name in pairs(action.build_env) do
                action_context.env[env_alias] = function (...)
                    return cobble.invoke_build_env(env_name, extra_tools, extra_build_envs, project_dir, table.pack(...))
                end
            end

            return action_context
        end

        invoke_action = function(action, action_context)
            if type(action[1]) == "function" then
                return action[1](action_context)
            else
                if #action.tool > 0 then
                    tool_alias, tool_name = next(action.tool)
                    return action_context.tool[tool_alias](table.unpack(action))
                elseif #action.build_env > 0 then
                    env_alias, env_name = next(action.build_env)
                    return action_context.env[env_alias](table.unpack(action))
                end
            end   
        end

        invoke_tool = function (name, extra_tools, extra_build_envs, project_dir, args)
            local action = cobble._tool_cache[name].action
            local action_context = create_action_context(action, extra_tools, extra_build_envs, project_dir, args)
            return invoke_action(action, action_context)
        end

        invoke_build_env = function (name, project_dir, args)
            local action = cobble._build_env_cache[name].action
            local action_context = create_action_context(action, extra_tools, extra_build_envs, project_dir, args)
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
    pub task_queue: Arc<(Mutex<Option<VecDeque<TargetJob>>>, Condvar)>,
    pub task_result_sender: Sender<TargetJobMessage>,
    pub cache: Arc<TaskExecutorCache>
}

fn run_task_executor_worker(args: TaskExecutorWorkerArgs) {
    let lua = create_lua_env(args.workspace_dir.as_path()).unwrap();
    init_lua_for_task_executor(&lua).unwrap();

    let db_env = lmdb::Environment::new().open(args.workspace_dir.join(".cobble.db").as_path()).unwrap();

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

        execute_target_task(args.workspace_dir.as_path(), &lua, &db_env, &next_task, args.task_result_sender.clone(), args.cache.clone());
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

fn ensure_tool_is_cached(lua: &mlua::Lua, tool: &ExternalTool) -> mlua::Result<()> {
    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let cached_tools: mlua::Table = cobble_table.get("_tool_cache")?;
    if !cached_tools.contains_key(tool.name.clone())? {
        cached_tools.set(tool.name.clone(), tool.clone())?;
    }

    Ok(())
}

fn ensure_build_env_is_cached(lua: &mlua::Lua, build_env: &BuildEnv) -> mlua::Result<()> {
    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let cached_build_envs: mlua::Table = cobble_table.get("_build_env_cache")?;
    if !cached_build_envs.contains_key(build_env.name.clone())? {
        let build_env_table = lua.create_table()?;
        build_env_table.set("action", build_env.action.clone())?;
        cached_build_envs.set(build_env.name.clone(), build_env_table)?;
    }

    Ok(())
}

fn execute_target<'lua>(lua: &'lua mlua::Lua, target: &TargetJob) -> mlua::Result<mlua::Value<'lua>> {
    // Make sure build envs and tools we need are 
    for (_, tool) in target.tools.iter() {
        ensure_tool_is_cached(lua, tool.as_ref())?;
    }

    for (_, env) in target.envs.iter() {
        ensure_build_env_is_cached(lua, env.as_ref())?;
    }

    let extra_tools: HashMap<&str, &str> = target.tools.iter()
        .map(|(t_alias, t)| (t_alias.as_str(), t.name.as_str()))
        .collect();

    let extra_build_envs: HashMap<&str, &str> = target.tools.iter()
        .map(|(e_alias, e)| (e_alias.as_str(), e.name.as_str()))
        .collect();

    let project_dir = target.target.dir.to_str()
        .ok_or_else(|| mlua::Error::runtime(format!("Unable to convert path to a UTF-8 string: {}", target.target.dir.display())))?;

    let mut args: mlua::Value = mlua::Value::Table(lua.create_table()?);
    for action in target.target.actions.iter() {
        let action_lua = lua.pack(action.clone())?;

        let cobble_table: mlua::Table = lua.globals().get("cobble")?;
        let create_action_context: mlua::Function = cobble_table.get("create_action_context")?;
        let action_context: mlua::Table = create_action_context.call((
            action_lua.clone(),
            extra_tools.clone(),
            extra_build_envs.clone(),
            target.target.dir.to_str(),
            project_dir.to_owned(),
            args.clone())
        )?;

        let invoke_action: mlua::Function = cobble_table.get("invoke_action")?;
        let action_result: mlua::Value = invoke_action.call((action_lua, action_context))?;
        args = action_result;
    }

    Ok(args)
}

fn execute_target_task(workspace_dir: &Path, lua: &mlua::Lua, db_env: &lmdb::Environment, task: &TargetJob, task_result_sender: Sender<TargetJobMessage>, cache: Arc<TaskExecutorCache>) {
    let mut up_to_date = true;
    let task_record_opt = match get_target_record(&db_env, task.target_name.as_str()) {
        Ok(r) => Some(r),
        Err(e) => match e {
            GetError::NotFound(_) => None,
            _ => { panic!("Error retrieving task record from the database"); }
        }
    };

    loop {
        let task_record = match task_record_opt {
            Some(r) => r,
            None => { up_to_date = false; break; }
        };
        
        if task.target.file_deps.len() != task_record.input.file_hashes.len() {
            up_to_date = false;
            break;
        }

        for file_dep in task.target.file_deps.iter() {
            let prev_hash = match task_record.input.file_hashes.get(file_dep) {
                Some(hash) => hash,
                None => { up_to_date = false; break; }
            };

            let cached_hash = cache.file_hashes.read().unwrap().get(file_dep).cloned();
            let current_hash = match cached_hash {
                Some(hash) => hash,
                None => {
                    let file_hash = compute_file_hash(workspace_dir.join(Path::new(file_dep)).as_path()).unwrap();
                    cache.file_hashes.write().unwrap().insert(file_dep.clone(), file_hash.clone());
                    file_hash
                }
            };

            if prev_hash != &current_hash {
                up_to_date = false;
                break;
            }
        }

        if task.target.target_deps.len() != task_record.input.task_outputs.len() {
            up_to_date = false;
            break;
        }

        for target_dep in task.target.target_deps.iter() {
            let prev_target_output = match task_record.input.task_outputs.get(target_dep) {
                Some(output) => output,
                None => { up_to_date = false; break; }
            };

            let cached_target_output = cache.target_outputs.read().unwrap().get(target_dep).cloned();
            let current_target_output = match cached_target_output {
                Some(output) => output,
                None => {
                    let target_record = get_target_record(&db_env, target_dep).unwrap();
                    cache.target_outputs.write().unwrap().insert(target_dep.clone(), target_record.output.clone());
                    target_record.output
                }
            };

            if prev_target_output != &current_target_output {
                up_to_date = false;
                break;
            }
        }

        break;
    }

    if up_to_date {
        task_result_sender.send(TargetJobMessage {
            target: task.target_name.clone(),
            message: TargetMessage::Complete(TargetResult::UpToDate)
        }).unwrap();
        return;
    }

    execute_target(lua, task).unwrap();
    task_result_sender.send(TargetJobMessage {
        target: task.target_name.clone(),
        message: TargetMessage::Complete(TargetResult::Success)
    }).unwrap();

}

#[cfg(test)]
mod tests {
    extern crate mktemp;

    use std::{collections::HashSet, sync::mpsc, time::Duration};

    use crate::{datamodel::ActionCmd, lua::detached_value::dump_function, workspace::query::WorkspaceTargetType};

    use super::*;

    #[test]
    fn test_execution_worker() {
        let tmpdir = mktemp::Temp::new_dir().unwrap();
        let workspace_dir = PathBuf::from(".");
        let lua = create_lua_env(workspace_dir.as_path()).unwrap();
        init_lua_for_task_executor(&lua).unwrap();
    
        let db_env = lmdb::Environment::new()
            .set_flags(lmdb::EnvironmentFlags::NO_SUB_DIR)
            .open(tmpdir.as_path().join(".cobble.db").as_path()).unwrap();
        let (tx, rx) = mpsc::channel::<TargetJobMessage>();

        let cache = Arc::new(TaskExecutorCache {
            file_hashes: RwLock::new(HashMap::new()),
            target_outputs: RwLock::new(HashMap::new())
        });

        let print_func: mlua::Function = lua.load(r#"function (c) print("Hi!", c) end"#).eval().unwrap();
        let print_tool = Arc::new(ExternalTool {
            name: String::from("print"),
            install: None,
            check: None,
            action: Action {
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Func(dump_function(print_func, &lua, &HashSet::new()).unwrap())
            }
        });

        let target = Arc::new(WorkspaceTarget {
            target_type: WorkspaceTargetType::Task,
            dir: workspace_dir.clone(),
            build_envs: HashMap::new(),
            tools: vec![(String::from("print"), String::from("print"))].into_iter().collect(),
            file_deps: Vec::new(),
            target_deps: Vec::new(),
            calc_deps: Vec::new(),
            actions: vec![Action {
                tools: vec![(String::from("print"), String::from("print"))].into_iter().collect(),
                build_envs: HashMap::new(),
                cmd: ActionCmd::Cmd(vec![String::from("There!")])
            }],
            artifacts: Vec::new()
        });

        let task_job = TargetJob {
            target_name: String::from("test"),
            tools: vec![(String::from("print"), print_tool)].into_iter().collect(),
            envs: HashMap::new(),
            target: target.clone()
        };

        execute_target_task(workspace_dir.as_path(), &lua, &db_env, &task_job, tx.clone(), cache.clone());

        let result = rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(result.target, "test");
        match result.message {
            TargetMessage::Complete(res) => match res {
                TargetResult::Success => { /* pass */ },
                _ => panic!("Did not get a success message")
            },
            _ => panic!("Did not get a completion message")
        };
    }
}

