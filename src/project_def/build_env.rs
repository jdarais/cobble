// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::{fmt, sync::Arc};

use crate::lua::s11n::SerLuaValueBlock;
use crate::project_def::action::validate_action;
use crate::project_def::tool::validate_vars_only_deps;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_required_key, with_prop,
};
use crate::project_def::Action;

use super::task::{dump_inline_task, validate_inline_task};
use super::validate::prop_path_string;
use super::TaskDef;

#[derive(Clone, Debug)]
pub enum EnvSetupTask {
    Ref(Arc<str>),
    Inline(TaskDef),
}

#[derive(Clone, Debug)]
pub struct BuildEnvDef {
    pub name: Arc<str>,
    pub setup_task: Option<EnvSetupTask>,
    pub action: Action,
    pub var_deps: Vec<Arc<str>>,
    pub ser_env: SerLuaValueBlock,
}

pub fn validate_build_env<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();
    match value {
        mlua::Value::Table(tbl_val) => {
            validate_required_key(tbl_val, "name", &mut prop_path)?;
            validate_required_key(tbl_val, "action", &mut prop_path)?;

            for pair in tbl_val.clone().pairs() {
                let (k, v): (mlua::Value, mlua::Value) = pair?;
                let k_str = validate_is_string(&k, &mut prop_path)?;
                match k_str.to_str()? {
                    "name" => with_prop(&mut prop_path, Cow::Borrowed("name"), |path| {
                        validate_is_string(&v, path).and(Ok(()))
                    }),
                    "deps" => with_prop(&mut prop_path, Cow::Borrowed("deps"), |path| {
                        validate_vars_only_deps(&v, path)
                    }),
                    "setup_task" => match v {
                        mlua::Value::String(_s) => Ok(()),
                        mlua::Value::Table(t) => {
                            with_prop(&mut prop_path, Cow::Borrowed("setup_task"), |path| {
                                validate_inline_task(lua, &mlua::Value::Table(t), path)
                            })
                        }
                        _ => Err(mlua::Error::runtime(format!(
                            "In {}: Expected a table or string for 'setup_task', but got a {}",
                            prop_path_string(&prop_path),
                            v.type_name()
                        ))),
                    },
                    "action" => with_prop(&mut prop_path, Cow::Borrowed("action"), |path| {
                        validate_action(lua, &v, path)
                    }),

                    s_str => key_validation_error(
                        s_str,
                        vec!["name", "setup_task", "action"],
                        &mut prop_path,
                    ),
                }?;
            }

            Ok(())
        }
        _ => Err(mlua::Error::runtime(format!(
            "Expected a table, but got a {}: {:?}",
            value.type_name(),
            value
        ))),
    }
}

impl fmt::Display for BuildEnvDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuildEnv(")?;
        write!(f, "name={}, ", &self.name)?;

        f.write_str("setup_task=")?;
        match &self.setup_task {
            Some(setup_task) => match setup_task {
                EnvSetupTask::Inline(t) => write!(f, "{}", t)?,
                EnvSetupTask::Ref(s) => write!(f, "\"{}\"", s)?,
            },
            None => {
                write!(f, "None")?;
            }
        };
        f.write_str(", ")?;

        write!(f, "action={})", self.action)
    }
}

impl<'lua> mlua::FromLua<'lua> for BuildEnvDef {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let ser_env = SerLuaValueBlock::from_lua(value.clone(), lua)?;
        let ser_env = ser_env.as_deterministic();
        match value {
            mlua::Value::Table(tbl) => {
                let name_str: String = tbl.get("name")?;
                let name = Arc::<str>::from(name_str);

                let mut var_deps: Vec<Arc<str>> = Vec::new();

                let deps_tbl_opt: Option<mlua::Table> = tbl.get("deps")?;
                if let Some(deps_tbl) = deps_tbl_opt {
                    let var_deps_tbl_opt: Option<mlua::Table> = deps_tbl.get("vars")?;
                    if let Some(var_deps_tbl) = var_deps_tbl_opt {
                        for pair in var_deps_tbl.clone().pairs() {
                            let (_k, v): (mlua::Value, String) = pair?;

                            var_deps.push(v.into());
                        }
                    }
                }

                let setup_task_val: mlua::Value = tbl.get("setup_task")?;
                let setup_task = match setup_task_val {
                    mlua::Value::String(s) => {
                        Some(EnvSetupTask::Ref(s.to_str()?.to_owned().into()))
                    }
                    mlua::Value::Table(t) => Some(EnvSetupTask::Inline(dump_inline_task(
                        lua,
                        name.clone(),
                        t,
                    )?)),
                    mlua::Value::Nil => None,
                    val => {
                        return Err(mlua::Error::runtime(format!("Expected table, string, or nil for 'setup_task' property, but got a {}", val.type_name())));
                    }
                };

                let action: Action = tbl.get("action")?;

                Ok(BuildEnvDef {
                    name,
                    setup_task,
                    action,
                    var_deps,
                    ser_env,
                })
            }
            val => {
                return Err(mlua::Error::runtime(format!(
                    "Unable to convert value to a BuildEnvDef: {:?}",
                    val
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use crate::lua::lua_env::create_lua_env;

    #[test]
    fn test_build_env_def_from_lua_table() {
        let lua = create_lua_env(Path::new(".")).unwrap();

        let build_env_table: mlua::Table = lua
            .load(
                r#"
                    {
                        name = "poetry",
                        install = {
                            {"poetry", "lock"},
                            {"poetry", "install"}
                        },
                        deps = {
                            files = {"pyproject.toml", "poetry.lock"}
                        },
                        action = function (args) cmd("poetry", table.unpack(args)) end
                    }
                "#,
            )
            .eval()
            .unwrap();

        let build_env: BuildEnvDef = lua.unpack(mlua::Value::Table(build_env_table)).unwrap();
        assert_eq!(build_env.name, Arc::<str>::from("poetry"));
    }
}
