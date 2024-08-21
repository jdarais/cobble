// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::config::TaskOutputCondition;
use crate::project_def::action::validate_action_list;
use crate::project_def::dependency::{validate_dep_list, Dependencies};
use crate::project_def::artifact::{validate_artifacts, Artifacts};
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_bool, validate_is_string,
    validate_is_table, validate_required_key,
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

fn validate_output_condition<'lua>(
    prop_name: Option<Cow<'static, str>>,
    value: &mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let val_str = validate_is_string(value, prop_name, prop_path)?;

    match val_str.to_str()? {
        "always" | "never" | "on_fail" => Ok(()),
        invalid_val => Err(mlua::Error::runtime(format!("Invalid value given for output condition: {}.  Expected one of [always, never, on_fail].", invalid_val)))
    }
}

fn validate_env_table<'lua>(
    prop_name: Option<Cow<'static, str>>,
    table: &mlua::Table,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path.as_mut());
    let mut has_build_env = false;
    for pair in table.clone().pairs() {
        if has_build_env {
            return Err(mlua::Error::runtime(
                "Only one env is allowed at the task level",
            ));
        }

        let (env_alias, env_name): (mlua::Value, mlua::Value) = pair?;
        validate_is_string(&env_alias, None, prop_path.as_mut())?;
        validate_is_string(&env_name, None, prop_path.as_mut())?;
        has_build_env = true;
    }
    Ok(())
}

pub fn validate_inline_task<'lua>(
    lua: &'lua mlua::Lua,
    prop_name: Option<Cow<'static, str>>,
    value: &mlua::Value<'lua>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    let tbl_val = validate_is_table(value, None, prop_path.as_mut())?;

    validate_required_key(tbl_val, "actions", None, prop_path.as_mut())?;

    for pair in tbl_val.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, None, prop_path.as_mut())?;
        match k_str.to_str()? {
            "name" => {
                validate_is_string(&v, Some(Cow::Borrowed("name")), prop_path.as_mut()).and(Ok(()))
            }
            "default" => {
                validate_is_bool(&v, Some(Cow::Borrowed("default")), prop_path.as_mut()).and(Ok(()))
            }
            "always_run" => {
                validate_is_bool(&v, Some(Cow::Borrowed("always_run")), prop_path.as_mut())
                    .and(Ok(()))
            }
            "interactive" => {
                validate_is_bool(&v, Some(Cow::Borrowed("interactive")), prop_path.as_mut())
                    .and(Ok(()))
            }
            "stdout" => {
                validate_output_condition(Some(Cow::Borrowed("stdout")), &v, prop_path.as_mut())
            }
            "stderr" => {
                validate_output_condition(Some(Cow::Borrowed("stderr")), &v, prop_path.as_mut())
            }
            "output" => {
                validate_output_condition(Some(Cow::Borrowed("output")), &v, prop_path.as_mut())
            }
            "env" => match v {
                mlua::Value::String(_) => Ok(()),
                mlua::Value::Table(t) => {
                    validate_env_table(Some(Cow::Borrowed("env")), &t, prop_path.as_mut())
                }
                _ => Err(mlua::Error::runtime(format!(
                    "Expected a string or table, but got a {}: {:?}",
                    v.type_name(),
                    v
                ))),
            },
            "actions" => {
                validate_action_list(lua, &v, Some(Cow::Borrowed("actions")), prop_path.as_mut())
            }
            "clean" => {
                validate_action_list(lua, &v, Some(Cow::Borrowed("clean")), prop_path.as_mut())
            }
            "deps" => validate_dep_list(lua, &v, Some(Cow::Borrowed("deps")), prop_path.as_mut()),
            "artifacts" => validate_artifacts(&v, Some(Cow::Borrowed("artifacts")), prop_path.as_mut()),
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
                prop_path.as_mut(),
            ),
        }?;
    }

    Ok(())
}

pub fn validate_task<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value<'lua>) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();

    let tbl_val = validate_is_table(value, None, prop_path.as_mut())?;
    validate_required_key(tbl_val, "name", None, prop_path.as_mut())?;

    validate_inline_task(lua, None, value, &mut prop_path)
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
