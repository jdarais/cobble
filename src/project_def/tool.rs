// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{borrow::Cow, fmt, sync::Arc};

use crate::lua::s11n::SerLuaValueBlock;
use crate::project_def::action::validate_action;
use crate::project_def::validate::{
    key_validation_error, validate_is_string, validate_is_table, validate_required_key, with_prop,
};
use crate::project_def::Action;

use super::validate::validate_table_has_only_string_or_sequence_keys;

#[derive(Clone, Debug)]
pub struct ExternalTool {
    pub name: Arc<str>,
    pub check: Option<Action>,
    pub action: Action,
    pub var_deps: Vec<Arc<str>>,
    pub ser_tool: SerLuaValueBlock,
}

pub fn validate_vars_only_deps<'lua>(
    value: &mlua::Value,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let deps_tbl = validate_is_table(&value, prop_path.as_mut())?;

    for pair in deps_tbl.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, prop_path.as_mut())?;
        match k_str.to_str()? {
            "vars" => with_prop(&mut *prop_path, Cow::Borrowed("vars"), |path| {
                let v_tbl = validate_is_table(&v, &mut *path)?;
                validate_table_has_only_string_or_sequence_keys(&v_tbl, path)
            }),
            unknown_key => key_validation_error(unknown_key, vec!["vars"], prop_path),
        }?;
    }

    Ok(())
}

pub fn validate_tool<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();

    let tool_tbl = validate_is_table(&value, &mut prop_path)?;

    validate_required_key(&tool_tbl, "name", &mut prop_path)?;
    validate_required_key(&tool_tbl, "action", &mut prop_path)?;

    for pair in tool_tbl.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        let k_str = validate_is_string(&k, &mut prop_path)?;
        match k_str.to_str()? {
            "name" => with_prop(&mut prop_path, Cow::Borrowed("name"), |path| {
                validate_is_string(&v, path).and(Ok(()))
            }),
            "deps" => with_prop(&mut prop_path, Cow::Borrowed("deps"), |path| {
                validate_vars_only_deps(&v, path)
            }),
            "check" => with_prop(&mut prop_path, Cow::Borrowed("check"), |path| {
                validate_action(lua, &v, path)
            }),
            "action" => with_prop(&mut prop_path, Cow::Borrowed("action"), |path| {
                validate_action(lua, &v, path)
            }),
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
        lua: &'lua mlua::prelude::Lua,
    ) -> mlua::prelude::LuaResult<Self> {
        let ser_tool = SerLuaValueBlock::from_lua(value.clone(), lua)?;
        let ser_tool = ser_tool.as_deterministic();
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
                    var_deps,
                    ser_tool,
                })
            }
            _ => Err(mlua::Error::runtime(format!(
                "Unable to convert value to action: {:?}",
                &value
            ))),
        }
    }
}
