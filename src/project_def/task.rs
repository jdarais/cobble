use std::borrow::Cow;
use std::{collections::HashMap, fmt, sync::Arc};

use crate::project_def::action::validate_action_list;
use crate::project_def::dependency::{validate_dep_list, Dependencies};
use crate::project_def::validate::{
    key_validation_error, push_prop_name_if_exists, validate_is_bool, validate_is_string,
    validate_is_table, validate_required_key, validate_table_is_sequence,
};
use crate::project_def::{Action, Artifact};

#[derive(Clone, Debug)]
pub struct TaskDef {
    pub name: Arc<str>,
    pub is_default: Option<bool>,
    pub always_run: Option<bool>,
    pub build_env: Option<(Arc<str>, Arc<str>)>,
    pub actions: Vec<Action>,
    pub clean: Vec<Action>,
    pub deps: Dependencies,
    pub artifacts: Vec<Artifact>,
}

pub fn validate_task<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value<'lua>) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();

    let tbl_val = validate_is_table(value, None, &mut prop_path)?;

    validate_required_key(tbl_val, "name", None, &mut prop_path)?;
    validate_required_key(tbl_val, "actions", None, &mut prop_path)?;

    for pair in tbl_val.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, None, &mut prop_path)?;
        match k_str.to_str()? {
            "name" => {
                validate_is_string(&v, Some(Cow::Borrowed("name")), &mut prop_path).and(Ok(()))
            }
            "default" => {
                validate_is_bool(&v, Some(Cow::Borrowed("default")), &mut prop_path).and(Ok(()))
            }
            "always_run" => {
                validate_is_bool(&v, Some(Cow::Borrowed("always_run")), &mut prop_path).and(Ok(()))
            }
            "env" => match v {
                mlua::Value::String(_) => Ok(()),
                mlua::Value::Table(t) => {
                    let mut prop_path =
                        push_prop_name_if_exists(Some(Cow::Borrowed("env")), &mut prop_path);
                    let mut has_build_env = false;
                    for pair in t.pairs() {
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
                _ => Err(mlua::Error::runtime(format!(
                    "Expected a string or table, but got a {}: {:?}",
                    v.type_name(),
                    v
                ))),
            },
            "actions" => {
                validate_action_list(lua, &v, Some(Cow::Borrowed("actions")), &mut prop_path)
            }
            "clean" => validate_action_list(lua, &v, Some(Cow::Borrowed("clean")), &mut prop_path),
            "deps" => validate_dep_list(lua, &v, Some(Cow::Borrowed("deps")), &mut prop_path),
            "artifacts" => {
                let artifacts_tbl =
                    validate_is_table(&v, Some(Cow::Borrowed("artifacts")), &mut prop_path)?;
                validate_table_is_sequence(
                    artifacts_tbl,
                    Some(Cow::Borrowed("artifacts")),
                    &mut prop_path,
                )?;
                for v in artifacts_tbl.clone().sequence_values() {
                    let artifact: mlua::Value = v?;
                    validate_is_string(
                        &artifact,
                        Some(Cow::Borrowed("artifacts")),
                        &mut prop_path,
                    )?;
                }
                Ok(())
            }
            unknown_key => key_validation_error(
                unknown_key,
                vec![
                    "name",
                    "default",
                    "always_run",
                    "env",
                    "actions",
                    "clean",
                    "deps",
                    "artifacts",
                ],
                &prop_path,
            ),
        }?;
    }

    Ok(())
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

        f.write_str("artifacts=[")?;
        for (i, artifact) in self.artifacts.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", artifact)?;
        }
        f.write_str("])")
    }
}

impl<'lua> mlua::FromLua<'lua> for TaskDef {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(task_table) => {
                let name_str: String = task_table.get("name")?;
                let name = Arc::<str>::from(name_str);

                let is_default: Option<bool> = task_table.get("default")?;
                let always_run: Option<bool> = task_table.get("always_run")?;

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
                let artifacts_opt: Option<Vec<Artifact>> = task_table.get("artifacts")?;
                let artifacts = artifacts_opt.unwrap_or_default();

                Ok(TaskDef {
                    name,
                    is_default,
                    always_run,
                    build_env,
                    actions,
                    clean,
                    deps,
                    artifacts,
                })
            }
            _ => Err(mlua::Error::runtime(format!(
                "Unable to convert value to Task: {:?}",
                value
            ))),
        }
    }
}
