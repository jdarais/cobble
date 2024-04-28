extern crate serde_json;
extern crate serde;

use std::{collections::HashMap, fmt};

use lmdb::Transaction;
use serde::{Serialize, Deserialize};

const TARGET_KEY_PREFIX: &str = "target:";



#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TargetInput {
    pub file_hashes: HashMap<String, Vec<u8>>,
    pub task_outputs: HashMap<String, serde_json::Value>
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TargetRecord {
    pub input: TargetInput,
    pub output: serde_json::Value
}


fn get_target_key(target_name: &str) -> String {
    let mut key = String::with_capacity(TARGET_KEY_PREFIX.len() + target_name.len());
    key.push_str(TARGET_KEY_PREFIX);
    key.push_str(target_name);
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

pub fn get_target_record(db_env: &lmdb::Environment, target_name: &str) -> Result<TargetRecord, GetError> {
    let db = db_env.open_db(None).map_err(|e| GetError::DBError(e))?;
    let tx = db_env.begin_ro_txn().map_err(|e| GetError::DBError(e))?;
    
    let target_key = get_target_key(target_name);
    let target_record_data = tx.get(db, &target_key)
        .map_err(|e| match e {
            lmdb::Error::NotFound => GetError::NotFound(target_key),
            _ => GetError::DBError(e)
        })?;

    serde_json::from_slice(target_record_data).map_err(|e| GetError::ParseError(e))
}
