use std::borrow::Cow;
use std::{fmt, sync::Arc};

use crate::project_def::action::validate_action;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_required_key,
};
use crate::project_def::Action;

use super::task::{dump_inline_task, validate_inline_task};
use super::validate::prop_path_string;
use super::TaskDef;

#[derive(Clone, Debug)]
pub enum EnvSetupTask {
    Ref(Arc<str>),
    Inline(TaskDef)
}

#[derive(Clone, Debug)]
pub struct BuildEnvDef {
    pub name: Arc<str>,
    pub setup_task: Option<EnvSetupTask>,
    pub action: Action,
}

pub fn validate_build_env<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();
    match value {
        mlua::Value::Table(tbl_val) => {
            validate_required_key(tbl_val, "name", None, &mut prop_path)?;
            validate_required_key(tbl_val, "action", None, &mut prop_path)?;

            for pair in tbl_val.clone().pairs() {
                let (k, v): (mlua::Value, mlua::Value) = pair?;
                let k_str = validate_is_string(&k, None, &mut prop_path)?;
                match k_str.to_str()? {
                    "name" => validate_is_string(&v, Some(Cow::Borrowed("name")), &mut prop_path)
                        .and(Ok(())),
                    "setup_task" => match v {
                        mlua::Value::String(_s) => { Ok(()) }
                        mlua::Value::Table(t) => validate_inline_task(lua, Some(Cow::Borrowed("setup_task")), &mlua::Value::Table(t), &mut prop_path),
                        _ => Err(mlua::Error::runtime(format!("In {}: Expected a table or string for 'setup_task', but got a {}", prop_path_string(&prop_path), v.type_name())))
                    },
                    "action" => {
                        validate_action(lua, &v, Some(Cow::Borrowed("action")), &mut prop_path)
                    }

                    s_str => key_validation_error(
                        s_str,
                        vec![
                            "name",
                            "setup_task",
                            "action",
                        ],
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
                EnvSetupTask::Ref(s) => write!(f, "\"{}\"", s)?
            }
            None => { write!(f, "None")?; }
        };
        f.write_str(", ")?;

        write!(f, "action={})", self.action)
    }
}

impl<'lua> mlua::FromLua<'lua> for BuildEnvDef {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let name_str: String = tbl.get("name")?;
                let name = Arc::<str>::from(name_str);

                let setup_task_val: mlua::Value = tbl.get("setup_task")?;
                let setup_task = match setup_task_val {
                    mlua::Value::String(s) => Some(EnvSetupTask::Ref(s.to_str()?.to_owned().into())),
                    mlua::Value::Table(t) => Some(EnvSetupTask::Inline(dump_inline_task(name.clone(), t)?)),
                    mlua::Value::Nil => None,
                    val => { return Err(mlua::Error::runtime(format!("Expected table, string, or nil for 'setup_task' property, but got a {}", val.type_name()))); }
                };

                let action: Action = tbl.get("action")?;

                Ok(BuildEnvDef {
                    name,
                    setup_task,
                    action,
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
