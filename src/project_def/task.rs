// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::config::TaskOutputCondition;
use crate::lua::s11n::{SerLuaTableRef, SerLuaValueBlock, SerLuaValueRef, SerLuaValueType};
use crate::project_def::action::validate_action_list;
use crate::project_def::artifact::{validate_artifacts, Artifacts};
use crate::project_def::dependency::{validate_dep_list, Dependencies};
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_bool, validate_is_string,
    validate_is_table, validate_required_key, with_prop, ValidationError,
};
use crate::project_def::Action;

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: Arc<str>,
    pub is_default: Option<bool>,
    pub always_run: Option<bool>,
    pub is_interactive: Option<bool>,
    pub show_stdout: Option<TaskOutputCondition>,
    pub show_stderr: Option<TaskOutputCondition>,
    pub build_env: Option<(Arc<str>, Arc<str>)>,
    pub actions: Vec<Action>,
    pub clean: Vec<Action>,
    pub deps: Dependencies,
    pub artifacts: Artifacts,
}

fn validate_output_condition<'a>(
    value: SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let val_str = validate_is_string(&value, prop_path)?;

    match val_str {
        "always" | "never" | "on_fail" => Ok(()),
        _ => Err(ValidationError::InvalidValue {
            path: prop_path
                .iter()
                .map(|v| SerLuaValueBlock::from(v.clone()))
                .collect(),
            value: value.into(),
            message: String::from("Expected one of [\"always\", \"never\", \"on_fail\"]"),
        }),
    }
}

fn validate_env_table<'a>(
    table: &SerLuaTableRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let mut has_build_env = false;
    for (k, v) in table.entries() {
        if has_build_env {
            return Err(ValidationError::InvalidValue {
                path: prop_path.iter().cloned().map(|v| v.into()).collect(),
                value: SerLuaValueRef::Table(table.clone()).into(),
                message: String::from("Only one env is allowed at the task level"),
            });
        }

        validate_is_string(&k, prop_path.as_mut())?;
        with_prop(prop_path, k, |path| validate_is_string(&v, path))?;
        has_build_env = true;
    }
    Ok(())
}

pub fn validate_inline_task<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let tbl_val = validate_is_table(value, prop_path)?;

    validate_required_key(tbl_val, "actions", prop_path)?;

    for (k, v) in tbl_val.entries() {
        let k_str = validate_is_string(&k, prop_path)?;
        let res = match k_str {
            "name" => with_prop(&mut *prop_path, SerLuaValueRef::String("name"), |path| {
                validate_is_string(&v, path).and(Ok(()))
            }),
            "default" => with_prop(&mut *prop_path, SerLuaValueRef::String("default"), |path| {
                validate_is_bool(&v, path).and(Ok(()))
            }),
            "always_run" => with_prop(
                &mut *prop_path,
                SerLuaValueRef::String("always_run"),
                |path| validate_is_bool(&v, path).and(Ok(())),
            ),
            "interactive" => with_prop(
                &mut *prop_path,
                SerLuaValueRef::String("interactive"),
                |path| validate_is_bool(&v, path).and(Ok(())),
            ),
            "stdout" => with_prop(&mut *prop_path, SerLuaValueRef::String("stdout"), |path| {
                validate_output_condition(v, path)
            }),
            "stderr" => with_prop(&mut *prop_path, SerLuaValueRef::String("stderr"), |path| {
                validate_output_condition(v, path)
            }),
            "output" => with_prop(&mut *prop_path, SerLuaValueRef::String("output"), |path| {
                validate_output_condition(v, path)
            }),
            "env" => match v {
                SerLuaValueRef::String(_) => Ok(()),
                SerLuaValueRef::Table(t) => {
                    with_prop(&mut *prop_path, SerLuaValueRef::String("env"), |path| {
                        validate_env_table(&t, path)
                    })
                }
                _ => with_prop(&mut *prop_path, SerLuaValueRef::String("env"), |path| {
                    Err(ValidationError::InvalidType {
                        path: path.iter().cloned().map(|p| p.into()).collect(),
                        expected: vec![SerLuaValueType::String, SerLuaValueType::Table],
                        actual: v.value_type(),
                        value: v.into(),
                    })
                }),
            },
            "actions" => with_prop(&mut *prop_path, SerLuaValueRef::String("actions"), |path| {
                validate_action_list(&v, path)
            }),
            "clean" => with_prop(&mut *prop_path, SerLuaValueRef::String("clean"), |path| {
                validate_action_list(&v, path)
            }),
            "deps" => with_prop(&mut *prop_path, SerLuaValueRef::String("deps"), |path| {
                validate_dep_list(&v, path)
            }),
            "artifacts" => with_prop(
                &mut *prop_path,
                SerLuaValueRef::String("artifacts"),
                |path| validate_artifacts(&v, path),
            ),
            _ => Err(ValidationError::InvalidKey {
                path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                expected: vec![
                    String::from("name"),
                    String::from("default"),
                    String::from("always_run"),
                    String::from("interactive"),
                    String::from("stdout"),
                    String::from("stderr"),
                    String::from("output"),
                    String::from("env"),
                    String::from("actions"),
                    String::from("clean"),
                    String::from("deps"),
                    String::from("artifacts"),
                ],
                key: k.into(),
                table: value.clone().into(),
            }),
        };
        res?;
    }

    Ok(())
}

pub fn validate_task<'a>(value: &SerLuaValueRef<'a>) -> Result<(), ValidationError> {
    let mut prop_path: Vec<SerLuaValueRef<'a>> = Vec::new();

    let tbl_val = validate_is_table(value, &mut prop_path)?;
    validate_required_key(tbl_val, "name", &mut prop_path)?;

    validate_inline_task(value, &mut prop_path)
}

impl fmt::Display for TaskDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Task(")?;
        write!(f, "name=\"{}\", ", self.name)?;

        if let Some((env_alias, env_name)) = &self.build_env {
            write!(f, "env={{\"{}\": \"{}\"}}, ", env_alias, env_name)?;
        }

        f.write_str("actions=[")?;
        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", action)?;
        }
        f.write_str("], ")?;

        write!(f, "deps={},", self.deps)?;

        write!(f, "artifacts={}", self.artifacts)
    }
}

pub fn dump_inline_task<'lua>(
    task_name: Arc<str>,
    task_table: mlua::Table<'lua>,
) -> mlua::Result<TaskDef> {
    let is_default: Option<bool> = task_table.get("default")?;
    let always_run: Option<bool> = task_table.get("always_run")?;
    let is_interactive: Option<bool> = task_table.get("interactive")?;

    let stdout: Option<TaskOutputCondition> = task_table.get("stdout")?;
    let stderr: Option<TaskOutputCondition> = task_table.get("stderr")?;
    let output: Option<TaskOutputCondition> = task_table.get("output")?;

    let build_env_val: mlua::Value = task_table.get("env")?;
    let build_env = match build_env_val {
        mlua::Value::String(s) => {
            let build_env_name = Arc::<str>::from(s.to_str()?);
            Some((build_env_name.clone(), build_env_name))
        }
        mlua::Value::Table(t) => {
            let mut envs: HashMap<Arc<str>, Arc<str>> = HashMap::new();
            for pair in t.pairs() {
                let (k, v): (String, String) = pair?;
                envs.insert(k.into(), v.into());
            }

            if envs.len() > 1 {
                return Err(mlua::Error::runtime(
                    "Only one build env can be assigned at the task level",
                ));
            }

            envs.into_iter().next()
        }
        mlua::Value::Nil => None,
        _ => {
            return Err(mlua::Error::runtime(format!(
                "Invalid type for env. Expected table, string, or nil: {:?}",
                build_env_val
            )));
        }
    };

    let actions: Vec<Action> = task_table.get("actions")?;
    let clean_opt: Option<Vec<Action>> = task_table.get("clean")?;
    let clean = clean_opt.unwrap_or_default();
    let deps_opt: Option<Dependencies> = task_table.get("deps")?;
    let deps = deps_opt.unwrap_or_default();
    let artifacts_opt: Option<Artifacts> = task_table.get("artifacts")?;
    let artifacts = artifacts_opt.unwrap_or_default();

    Ok(TaskDef {
        name: task_name,
        is_default,
        always_run,
        is_interactive,
        show_stdout: stdout.or(output.clone()),
        show_stderr: stderr.or(output),
        build_env,
        actions,
        clean,
        deps,
        artifacts,
    })
}

impl<'lua> mlua::FromLua<'lua> for TaskDef {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(task_table) => {
                let name_str: String = task_table.get("name")?;
                let name = Arc::<str>::from(name_str);

                dump_inline_task(name, task_table)
            }
            _ => Err(mlua::Error::runtime(format!(
                "Unable to convert value to Task: {:?}",
                value
            ))),
        }
    }
}
