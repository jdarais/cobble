extern crate mlua;

use std::fmt;

use crate::datamodel::Action;

#[derive(Clone, Debug)]
pub struct ExternalTool {
    pub name: String,
    pub install: Option<Action>,
    pub check: Option<Action>,
    pub action: Action
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

impl <'lua> mlua::FromLua<'lua> for ExternalTool {
    fn from_lua(value: mlua::prelude::LuaValue<'lua>, _lua: &'lua mlua::prelude::Lua) -> mlua::prelude::LuaResult<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let name: String = tbl.get("name")?;

                let install: Option<Action> = tbl.get("install")?;
                if let Some(ins) = &install {
                    if ins.build_envs.len() > 0 {
                        return Err(mlua::Error::runtime("External tools cannot depend on build environments"));
                    }
                }

                let check: Option<Action> = tbl.get("install")?;
                if let Some(chk) = &check {
                    if chk.build_envs.len() > 0 {
                        return Err(mlua::Error::runtime("External tools cannot depend on build environments"));
                    }
                }

                let action: Action = tbl.get("action")?;
                if action.build_envs.len() > 0 {
                    return Err(mlua::Error::runtime("External tools cannot depend on build environments"));
                }

                Ok(ExternalTool { name, install, check, action })
            },
            mlua::Value::UserData(val) => Ok(val.borrow::<ExternalTool>()?.clone()),
            _ => Err(mlua::Error::runtime(format!("Unable to convert value to action: {:?}", &value)))
        }
    }
}
