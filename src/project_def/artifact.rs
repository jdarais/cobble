// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::fmt;
use std::sync::Arc;

use crate::lua::s11n::{SerLuaValueBlock, SerLuaValueRef, SerLuaValueType};
use crate::project_def::validate::{
    validate_is_string, validate_table_value_type, with_prop, ValidationError,
};

use super::validate::{
    validate_is_table, validate_table_has_only_string_or_sequence_keys, validate_table_is_sequence,
};

#[derive(Clone, Debug, Default)]
pub struct Artifacts {
    pub files: Vec<Arc<str>>,
    pub calc: Vec<Arc<str>>,
}

impl fmt::Display for Artifacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Artifact(files=[")?;
        for (i, filename) in self.files.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            f.write_str(filename.as_ref())?;
        }
        f.write_str("], calc=[")?;
        for (i, calc) in self.calc.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            f.write_str(calc.as_ref())?;
        }
        f.write_str("])")?;

        Ok(())
    }
}

pub fn validate_artifacts<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let table_value = validate_is_table(value, &mut *prop_path)?;

    let string_type = vec![SerLuaValueType::String];
    for (k, v) in table_value.entries() {
        let res = match k {
            SerLuaValueRef::String(k_str) => match k_str {
                "files" => with_prop(&mut *prop_path, SerLuaValueRef::String("files"), |path| {
                    let files_table = validate_is_table(&v, &mut *path)?;
                    validate_table_has_only_string_or_sequence_keys(files_table, &mut *path)?;
                    validate_table_value_type(files_table, &string_type, &mut *path)
                }),
                "calc" => with_prop(&mut *prop_path, SerLuaValueRef::String("calc"), |path| {
                    let calc_table = validate_is_table(&v, &mut *path)?;
                    validate_table_is_sequence(calc_table, &mut *path)?;
                    validate_table_value_type(calc_table, &string_type, &mut *path)
                }),
                _ => Err(ValidationError::InvalidKey {
                    path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                    expected: vec![String::from("files"), String::from("calc")],
                    key: k.into(),
                    table: value.clone().into(),
                }),
            },
            SerLuaValueRef::Integer(_) => with_prop(&mut *prop_path, k, |path| {
                validate_is_string(&v, path).and(Ok(()))
            }),
            _ => Err(ValidationError::InvalidType {
                path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                expected: vec![SerLuaValueType::String, SerLuaValueType::Integer],
                actual: k.value_type(),
                value: k.into(),
            }),
        };
        res?;
    }

    Ok(())
}

impl<'lua> mlua::FromLua<'lua> for Artifacts {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(table_value) => {
                let mut files: Vec<Arc<str>> = Vec::new();
                let mut calc: Vec<Arc<str>> = Vec::new();

                let files_value_opt: Option<Vec<String>> = table_value.get("files")?;
                if let Some(files_value) = files_value_opt {
                    for f in files_value {
                        files.push(Arc::<str>::from(f));
                    }
                }

                let calc_value_opt: Option<Vec<String>> = table_value.get("calc")?;
                if let Some(calc_value) = calc_value_opt {
                    for c in calc_value {
                        calc.push(Arc::<str>::from(c));
                    }
                }

                // Treat sequence values as files
                for val in table_value.sequence_values() {
                    let f: String = val?;
                    files.push(Arc::<str>::from(f));
                }

                Ok(Artifacts { files, calc })
            }
            _ => Err(mlua::Error::RuntimeError(format!(
                "Expected a table, but got a {}",
                value.type_name()
            ))),
        }
    }
}

impl TryFrom<&SerLuaValueBlock> for Artifacts {
    type Error = String;
    fn try_from(value: &SerLuaValueBlock) -> Result<Self, Self::Error> {
        let mut artifacts = Artifacts {
            files: Vec::new(),
            calc: Vec::new(),
        };

        let value_ref = SerLuaValueRef::from(&value.values, 0);

        let artifacts_table = match &value_ref {
            SerLuaValueRef::Table(t) => t,
            _ => {
                return Err(format!("Value is not a table"));
            }
        };

        let files_entry_opt = artifacts_table
            .entries()
            .find(|(k, _v)| *k == SerLuaValueRef::String("files"));
        let files_table_opt = match files_entry_opt {
            Some(ent) => match ent.1 {
                SerLuaValueRef::Table(t) => Some(t),
                _ => {
                    return Err(format!("Value is not a table"));
                }
            },
            None => None,
        };

        if let Some(files_table) = files_table_opt {
            for (_k, v) in files_table.entries() {
                let v_str = match v {
                    SerLuaValueRef::String(s) => s,
                    _ => {
                        return Err(format!("files values must be strings"));
                    }
                };

                artifacts.files.push(Arc::<str>::from(v_str.to_owned()));
            }
        }

        let calc_entry_opt = artifacts_table
            .entries()
            .find(|(k, _v)| *k == SerLuaValueRef::String("calc"));
        let calc_table_opt = match calc_entry_opt {
            Some(ent) => match ent.1 {
                SerLuaValueRef::Table(t) => Some(t),
                _ => {
                    return Err(format!("Value is not a talbe"));
                }
            },
            None => None,
        };

        if let Some(calc_table) = calc_table_opt {
            for (_k, v) in calc_table.entries() {
                let v_str = match v {
                    SerLuaValueRef::String(s) => s,
                    _ => {
                        return Err(format!("calc values must be strings"));
                    }
                };

                artifacts.calc.push(Arc::<str>::from(v_str.to_owned()));
            }
        }

        Ok(artifacts)
    }
}
