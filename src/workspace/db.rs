extern crate serde_json;
extern crate serde;

use std::{collections::HashMap, error::Error, fmt, io, path::Path};

use lmdb::{Transaction, WriteFlags};
use serde::{Serialize, Deserialize};

use crate::datamodel::types::TaskVar;

const TASK_KEY_PREFIX: &str = "task:";



#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskInput {
    pub project_source_hashes: HashMap<String, String>,
    pub file_hashes: HashMap<String, String>,
    pub task_outputs: HashMap<String, serde_json::Value>,
    pub vars: HashMap<String, TaskVar>
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskOutput {
    pub file_hashes: HashMap<String, String>,
    pub task_output: serde_json::Value
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskRecord {
    pub input: TaskInput,
    pub output: TaskOutput
}


fn get_task_key(task_name: &str) -> String {
    let mut key = String::with_capacity(TASK_KEY_PREFIX.len() + task_name.len());
    key.push_str(TASK_KEY_PREFIX);
    key.push_str(task_name);
    key
}

#[derive(Debug)]
pub enum GetError {
    ParseError(serde_json::Error),
    DBError(lmdb::Error),
    NotFound(String)
}

impl fmt::Display for GetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use GetError::*;
        match self {
            ParseError(e) => write!(f, "Error parsing record: {}", e),
            DBError(e) => write!(f, "Database error: {}", e),
            NotFound(key) => write!(f, "DB key not found: {}", key)
        }
    }
}

pub fn get_task_record(db_env: &lmdb::Environment, db: lmdb::Database, task_name: &str) -> Result<TaskRecord, GetError> {
    let task_key = get_task_key(task_name);
    
    let tx = db_env.begin_ro_txn().map_err(|e| GetError::DBError(e))?;
    let task_record_data = tx.get(db, &task_key)
        .map_err(|e| match e {
            lmdb::Error::NotFound => GetError::NotFound(task_key),
            _ => GetError::DBError(e)
        })?;

    serde_json::from_slice(task_record_data).map_err(|e| GetError::ParseError(e))
}

#[derive(Debug)]
pub enum PutError {
    SerializeError(serde_json::Error),
    DBError(lmdb::Error),
    FileError(io::Error)
}

impl Error for PutError {}
impl fmt::Display for PutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PutError::*;
        match self {
            SerializeError(e) => write!(f, "Error serializing record: {}", e),
            DBError(e) => write!(f, "Database error: {}", e),
            FileError(e) => write!(f, "File error: {}", e)
        }
    }
}

pub fn put_task_record(db_env: &lmdb::Environment, db: lmdb::Database, task_name: &str, record: &TaskRecord) -> Result<(), PutError> {
    let task_key = get_task_key(task_name);
    
    let serialized_record = serde_json::to_vec(record).map_err(|e| PutError::SerializeError(e))?;
    
    let mut tx = db_env.begin_rw_txn().map_err(|e| PutError::DBError(e))?;
    tx.put(db, &task_key, &serialized_record, WriteFlags::empty()).map_err(|e| PutError::DBError(e))?;
    tx.commit().map_err(|e| PutError::DBError(e))?;

    Ok(())
}

pub fn new_db_env(path: &Path) -> lmdb::Result<lmdb::Environment> {
    lmdb::Environment::new()
            .set_flags(lmdb::EnvironmentFlags::NO_SUB_DIR)
            .open(path)
}
