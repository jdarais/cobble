// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use mlua::FromLua;

use crate::config::TaskOutputCondition;
use crate::lua::s11n::SerLuaValueBlock;
use crate::project_def::action::validate_action_list;
use crate::project_def::artifact::{validate_artifacts, Artifacts};
use crate::project_def::dependency::{validate_dep_list, Dependencies};
use crate::project_def::validate::{
    key_validation_error, validate_is_bool, validate_is_string, validate_is_table,
    validate_required_key, with_prop,
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
    pub ser_task: SerLuaValueBlock
}

fn validate_output_condition<'lua>(
    value: &mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let val_str = validate_is_string(value, prop_path)?;

    match val_str.to_str()? {
        "always" | "never" | "on_fail" => Ok(()),
        invalid_val => Err(mlua::Error::runtime(format!("Invalid value given for output condition: {}.  Expected one of [always, never, on_fail].", invalid_val)))
    }
}

fn validate_env_table<'lua>(
    table: &mlua::Table,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut has_build_env = false;
    for pair in table.clone().pairs() {
        if has_build_env {
            return Err(mlua::Error::runtime(
                "Only one env is allowed at the task level",
            ));
        }

        let (env_alias, env_name): (mlua::Value, mlua::Value) = pair?;
        validate_is_string(&env_alias, &mut *prop_path)?;
        validate_is_string(&env_name, &mut *prop_path)?;
        has_build_env = true;
    }
    Ok(())
}

pub fn validate_inline_task<'lua>(
    lua: &'lua mlua::Lua,
    value: &mlua::Value<'lua>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let tbl_val = validate_is_table(value, &mut *prop_path)?;

    validate_required_key(tbl_val, "actions", &mut *prop_path)?;

    for pair in tbl_val.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, &mut *prop_path)?;
        match k_str.to_str()? {
            "name" => with_prop(&mut *prop_path, Cow::Borrowed("name"), |path| {
                validate_is_string(&v, path).and(Ok(()))
            }),
            "default" => with_prop(&mut *prop_path, Cow::Borrowed("default"), |path| {
                validate_is_bool(&v, path).and(Ok(()))
            }),
            "always_run" => with_prop(&mut *prop_path, Cow::Borrowed("always_run"), |path| {
                validate_is_bool(&v, path).and(Ok(()))
            }),
            "interactive" => with_prop(&mut *prop_path, Cow::Borrowed("interactive"), |path| {
                validate_is_bool(&v, path).and(Ok(()))
            }),
            "stdout" => with_prop(&mut *prop_path, Cow::Borrowed(""), |path| {
                validate_output_condition(&v, path)
            }),
            "stderr" => with_prop(&mut *prop_path, Cow::Borrowed(""), |path| {
                validate_output_condition(&v, path)
            }),
            "output" => with_prop(&mut *prop_path, Cow::Borrowed(""), |path| {
                validate_output_condition(&v, path)
            }),
            "env" => match v {
                mlua::Value::String(_) => Ok(()),
                mlua::Value::Table(t) => with_prop(&mut *prop_path, Cow::Borrowed("env"), |path| {
                    validate_env_table(&t, path)
                }),
                _ => Err(mlua::Error::runtime(format!(
                    "Expected a string or table, but got a {}: {:?}",
                    v.type_name(),
                    v
                ))),
            },
            "actions" => with_prop(&mut *prop_path, Cow::Borrowed("actions"), |path| {
                validate_action_list(lua, &v, path)
            }),
            "clean" => with_prop(&mut *prop_path, Cow::Borrowed("clean"), |path| {
                validate_action_list(lua, &v, path)
            }),
            "deps" => with_prop(&mut *prop_path, Cow::Borrowed("deps"), |path| {
                validate_dep_list(lua, &v, path)
            }),
            "artifacts" => with_prop(&mut *prop_path, Cow::Borrowed("artifacts"), |path| {
                validate_artifacts(&v, path)
            }),
            unknown_key => key_validation_error(
                unknown_key,
                vec![
                    "name",
                    "default",
                    "always_run",
                    "interactive",
                    "stdout",
                    "stderr",
                    "output",
                    "env",
                    "actions",
                    "clean",
                    "deps",
                    "artifacts",
                ],
                &mut *prop_path,
            ),
        }?;
    }

    Ok(())
}

pub fn validate_task<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value<'lua>) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();

    let tbl_val = validate_is_table(value, &mut prop_path)?;
    validate_required_key(tbl_val, "name", &mut prop_path)?;

    validate_inline_task(lua, value, &mut prop_path)
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
    lua: &'lua mlua::Lua,
    task_name: Arc<str>,
    task_table: mlua::Table<'lua>,
) -> mlua::Result<TaskDef> {
    let ser_task = SerLuaValueBlock::from_lua(mlua::Value::Table(task_table.clone()), lua)?;
    let ser_task = ser_task.as_deterministic();
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
        ser_task
    })
}

impl<'lua> mlua::FromLua<'lua> for TaskDef {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(task_table) => {
                let name_str: String = task_table.get("name")?;
                let name = Arc::<str>::from(name_str);

                dump_inline_task(lua, name, task_table)
            }
            _ => Err(mlua::Error::runtime(format!(
                "Unable to convert value to Task: {:?}",
                value
            ))),
        }
    }
}
