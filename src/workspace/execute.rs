extern crate lmdb;

use std::{collections::{HashMap, VecDeque}, fmt, sync::{mpsc::Sender, Arc, Condvar, Mutex}, thread::Thread};

use crate::{datamodel::{BuildEnv, ExternalTool, Task}, workspace::{dependency::ExecutionGraph, query::WorkspaceTargetRef}};

pub enum WorkspaceTarget {
    BuildEnv(Arc<BuildEnv>),
    Task(Arc<Task>)
}

pub struct TaskJob {
    tools: HashMap<String, Arc<ExternalTool>>,
    envs: HashMap<String, Arc<BuildEnv>>,
    target: WorkspaceTarget
}

pub struct TaskResult {

}

#[derive(Debug)]
pub enum CreateJobsError {
    TargetLookupError(String)
}

impl fmt::Display for CreateJobsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CreateJobsError::*;
        match self {
            TargetLookupError(t) => write!(f, "Target not found while creating jbos: {}", t)
        }
    }
}

// pub fn create_jobs(all_targets: &HashMap<&str, WorkspaceTargetRef>, execution_graph: &ExecutionGraph) -> Result<HashMap<String, TaskJob>, CreateJobsError> {
//     let mut cloned_targets: HashMap<String, WorkspaceTarget> = HashMap::new();

//     let mut cloned_tools: HashMap<String, Arc<ExternalTool>> = execution_graph.required_tools.iter().map(|(&k, v)| (String::from(k), Arc::new(v.clone()))).collect();

//     for (&target_name, target) in all_targets.iter() {
//         let cloned_target = match cloned_targets.get(target_name) {
//             Some(t) => t.clone(),
//             None => {
//                 let t_ref = all_targets.get(target_name).ok_or_else(|| CreateJobsError::TargetLookupError(String::from(target_name)))?;
//                 match t_ref {
//                     WorkspaceTargetRef::
//                 }
//             }
//         }
//     }
    
//     Ok(HashMap::new())
// }

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

struct TaskExecutorWorker {
    lua: mlua::Lua,
    task_queue: Arc<(Mutex<VecDeque<TaskJob>>, Condvar)>,
    task_result_sender: Sender<TaskResult>,
}
