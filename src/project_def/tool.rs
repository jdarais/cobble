// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::collections::HashMap;
use std::{borrow::Cow, fmt, sync::Arc};

use crate::project_def::action::validate_action;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_is_table, validate_required_key,
};
use crate::project_def::Action;
use crate::project_def::types::StringOrInt;

use super::validate::{push_prop_name_if_exists, validate_table_has_only_string_or_sequence_keys};

#[derive(Clone, Debug)]
pub struct ExternalTool {
    pub name: Arc<str>,
    pub check: Option<Action>,
    pub action: Action,
    pub var_deps: HashMap<Arc<str>, Arc<str>>,
}

fn validate_tool_deps<'lua>(
    value: &mlua::Value,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    let deps_tbl = validate_is_table(&value, None, prop_path.as_mut())?;

    for pair in deps_tbl.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, None, prop_path.as_mut())?;
        match k_str.to_str()? {
            "vars" => {
                let v_tbl = validate_is_table(&v, Some(Cow::Borrowed("vars")), prop_path.as_mut())?;
                validate_table_has_only_string_or_sequence_keys(&v_tbl, Some(Cow::Borrowed("vars")), prop_path.as_mut())
            },
            unknown_key => key_validation_error(unknown_key, vec!["vars"], prop_path.as_mut()),
        }?;
    }

    Ok(())
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
            "deps" => validate_tool_deps(&v, Some(Cow::Borrowed("deps")), &mut prop_path),
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

        f.write_str("deps={")?;
        write!(f, "vars={:?}", self.var_deps)?;
        f.write_str("}")?;

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

                let mut var_deps: HashMap<Arc<str>, Arc<str>> = HashMap::new();
                
                let deps_tbl_opt: Option<mlua::Table> = tbl.get("deps")?;
                if let Some(deps_tbl) = deps_tbl_opt {
                    let var_deps_tbl_opt: Option<mlua::Table> = deps_tbl.get("vars")?;
                    if let Some(var_deps_tbl) = var_deps_tbl_opt {
                        for pair in var_deps_tbl.clone().pairs() {
                            let (k, v): (StringOrInt, String) = pair?;
                            let var_dep_value: Arc<str> = v.into();
                            let var_dep_key = match k {
                                StringOrInt::Int(_i) => var_dep_value.clone(),
                                StringOrInt::String(s) => s.into()
                            };

                            var_deps.insert(var_dep_key, var_dep_value);
                        }
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
                    check,
                    action,
                    var_deps
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
            check,
            action,
            var_deps
        } = self;
        let tool_table = lua.create_table()?;

        tool_table.set("name", name.as_ref())?;

        if var_deps.len() > 0 {
            let deps_table = lua.create_table()?;

            let var_deps_table = lua.create_table()?;
            for (k, v) in var_deps {
                var_deps_table.set(k.as_ref(), v.as_ref())?;
            }

            deps_table.set("vars", var_deps_table)?;
        }

        if let Some(chk) = check {
            tool_table.set("check", chk)?;
        }

        tool_table.set("action", action)?;

        Ok(mlua::Value::Table(tool_table))
    }
}
