use std::{borrow::Cow, fmt, sync::Arc};

use crate::project_def::action::validate_action;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_is_table, validate_required_key,
};
use crate::project_def::Action;

#[derive(Clone, Debug)]
pub struct ExternalTool {
    pub name: Arc<str>,
    pub install: Option<Action>,
    pub check: Option<Action>,
    pub action: Action,
}

pub fn validate_tool<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();

    let tool_tbl = validate_is_table(&value, None, &mut prop_path)?;

    validate_required_key(&tool_tbl, "name", None, &mut prop_path)?;
    validate_required_key(&tool_tbl, "action", None, &mut prop_path)?;

    for pair in tool_tbl.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, None, &mut prop_path)?;
        match k_str.to_str()? {
            "name" => {
                validate_is_string(&v, Some(Cow::Borrowed("name")), &mut prop_path).and(Ok(()))
            }
            "install" => validate_action(lua, &v, Some(Cow::Borrowed("install")), &mut prop_path),
            "check" => validate_action(lua, &v, Some(Cow::Borrowed("check")), &mut prop_path),
            "action" => validate_action(lua, &v, Some(Cow::Borrowed("action")), &mut prop_path),
            unknown_key => key_validation_error(
                unknown_key,
                vec!["name", "install", "check", "action"],
                &prop_path,
            ),
        }?;
    }

    Ok(())
}

impl fmt::Display for ExternalTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExternalTool(")?;
        write!(f, "name=\"{}\", ", self.name)?;

        if let Some(install_action) = self.install.as_ref() {
            write!(f, "install={}, ", install_action)?;
        }

        if let Some(check_action) = self.check.as_ref() {
            write!(f, "check={}, ", check_action)?;
        }

        write!(f, "action={})", &self.action)
    }
}

impl<'lua> mlua::FromLua<'lua> for ExternalTool {
    fn from_lua(
        value: mlua::prelude::LuaValue<'lua>,
        _lua: &'lua mlua::prelude::Lua,
    ) -> mlua::prelude::LuaResult<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let name_str: String = tbl.get("name")?;
                let name = Arc::<str>::from(name_str);

                let install: Option<Action> = tbl.get("install")?;
                if let Some(ins) = &install {
                    if ins.build_envs.len() > 0 {
                        return Err(mlua::Error::runtime(
                            "External tools cannot depend on build environments",
                        ));
                    }
                }

                let check: Option<Action> = tbl.get("check")?;
                if let Some(chk) = &check {
                    if chk.build_envs.len() > 0 {
                        return Err(mlua::Error::runtime(
                            "External tools cannot depend on build environments",
                        ));
                    }
                }

                let action: Action = tbl.get("action")?;
                if action.build_envs.len() > 0 {
                    return Err(mlua::Error::runtime(
                        "External tools cannot depend on build environments",
                    ));
                }

                Ok(ExternalTool {
                    name,
                    install,
                    check,
                    action,
                })
            }
            _ => Err(mlua::Error::runtime(format!(
                "Unable to convert value to action: {:?}",
                &value
            ))),
        }
    }
}

impl<'lua> mlua::IntoLua<'lua> for ExternalTool {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let ExternalTool {
            name,
            install,
            check,
            action,
        } = self;
        let tool_table = lua.create_table()?;

        tool_table.set("name", name.as_ref())?;

        if let Some(inst) = install {
            tool_table.set("install", inst)?;
        }

        if let Some(chk) = check {
            tool_table.set("check", chk)?;
        }

        tool_table.set("action", action)?;

        Ok(mlua::Value::Table(tool_table))
    }
}
