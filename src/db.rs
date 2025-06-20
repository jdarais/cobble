// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{collections::BTreeMap, error::Error, fmt, io, path::Path};

use lmdb::{Transaction, WriteFlags};
use serde::{Deserialize, Serialize};

use crate::{
    lua::s11n::{SerLuaValue, SerLuaValueBlock},
};

const TASK_KEY_PREFIX: &str = "task:";

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct TaskInput {
    #[serde(default)]
    pub dir_mtimes: BTreeMap<String, u128>,

    #[serde(default)]
    pub file_hashes: BTreeMap<String, String>,

    #[serde(default)]
    pub task_outputs: BTreeMap<String, SerLuaValueBlock>,

    #[serde(default)]
    pub vars: serde_json::Map<String, serde_json::Value>,

    #[serde(default)]
    pub task_hash: String,

    #[serde(default)]
    pub env_hashes: BTreeMap<String, String>,

    #[serde(default)]
    pub tool_hashes: BTreeMap<String, String>,
}

fn default_ser_lua_value_block() -> SerLuaValueBlock {
    SerLuaValueBlock {
        values: vec![SerLuaValue::Nil],
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskOutput {
    #[serde(default)]
    pub file_hashes: BTreeMap<String, String>,

    #[serde(default = "default_ser_lua_value_block")]
    pub task_output: SerLuaValueBlock,
}

impl Default for TaskOutput {
    fn default() -> Self {
        Self {
            file_hashes: Default::default(),
            task_output: SerLuaValueBlock {
                values: vec![SerLuaValue::Nil],
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskRecord {
    #[serde(default)]
    pub input: TaskInput,

    #[serde(default)]
    pub output: TaskOutput,
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
    NotFound(String),
}

impl Error for GetError {}
impl fmt::Display for GetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use GetError::*;
        match self {
            ParseError(e) => write!(f, "Error parsing record: {}", e),
            DBError(e) => write!(f, "Database error: {}", e),
            NotFound(key) => write!(f, "DB key not found: {}", key),
        }
    }
}

pub fn get_task_record(
    db_env: &lmdb::Environment,
    db: lmdb::Database,
    task_name: &str,
) -> Result<TaskRecord, GetError> {
    let task_key = get_task_key(task_name);

    let tx = db_env.begin_ro_txn().map_err(|e| GetError::DBError(e))?;
    let task_record_data = tx.get(db, &task_key).map_err(|e| match e {
        lmdb::Error::NotFound => GetError::NotFound(task_key),
        _ => GetError::DBError(e),
    })?;

    serde_json::from_slice(task_record_data).map_err(|e| GetError::ParseError(e))
}

#[derive(Debug)]
pub enum PutError {
    SerializeError(serde_json::Error),
    DBError(lmdb::Error),
    FileError(io::Error),
}

impl Error for PutError {}
impl fmt::Display for PutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use PutError::*;
        match self {
            SerializeError(e) => write!(f, "Error serializing record: {}", e),
            DBError(e) => write!(f, "Database error: {}", e),
            FileError(e) => write!(f, "File error: {}", e),
        }
    }
}

pub fn put_task_record(
    db_env: &lmdb::Environment,
    db: lmdb::Database,
    task_name: &str,
    record: &TaskRecord,
) -> Result<(), PutError> {
    let task_key = get_task_key(task_name);

    let serialized_record = serde_json::to_vec(record).map_err(|e| PutError::SerializeError(e))?;

    let mut tx = db_env.begin_rw_txn().map_err(|e| PutError::DBError(e))?;
    tx.put(db, &task_key, &serialized_record, WriteFlags::empty())
        .map_err(|e| PutError::DBError(e))?;
    tx.commit().map_err(|e| PutError::DBError(e))?;

    Ok(())
}

#[derive(Debug)]
pub enum DeleteError {
    DBError(lmdb::Error),
    FileError(io::Error),
}

impl Error for DeleteError {}
impl fmt::Display for DeleteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use DeleteError::*;
        match self {
            DBError(e) => write!(f, "Database error: {}", e),
            FileError(e) => write!(f, "File error: {}", e),
        }
    }
}

pub fn delete_task_record(
    db_env: &lmdb::Environment,
    db: lmdb::Database,
    task_name: &str,
) -> Result<(), DeleteError> {
    let task_key = get_task_key(task_name);

    let mut tx = db_env.begin_rw_txn().map_err(|e| DeleteError::DBError(e))?;
    let res = tx.del(db, &task_key, None);

    if let Err(e) = res {
        match e {
            lmdb::Error::NotFound => { /* Ok */ }
            _ => {
                return Err(DeleteError::DBError(e));
            }
        }
    }

    tx.commit().map_err(|e| DeleteError::DBError(e))?;

    Ok(())
}

pub fn new_db_env(path: &Path, max_db_size: usize) -> lmdb::Result<lmdb::Environment> {
    lmdb::Environment::new()
        .set_flags(lmdb::EnvironmentFlags::NO_SUB_DIR)
        .set_map_size(max_db_size)
        .open(path)
}
