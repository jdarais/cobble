extern crate mlua;

use std::fmt;

use crate::cobble::datamodel::Action;

#[derive(Clone, Debug)]
pub struct ExternalTool {
    name: String,
    install_action: Option<Action>,
    check_action: Option<Action>,
    action: Action
}

impl fmt::Display for ExternalTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExternalTool(")?;
        write!(f, "name=\"{}\", ", self.name)?;

        if let Some(install_action) = self.install_action.as_ref() {
            write!(f, "install={}, ", install_action)?;
        }

        if let Some(check_action) = self.check_action.as_ref() {
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
                let install_action: Option<Action> = tbl.get("install")?;
                let check_action: Option<Action> = tbl.get("install")?;
                let action: Action = tbl.get("action")?;

                Ok(ExternalTool { name, install_action, check_action, action })
            },
            mlua::Value::UserData(val) => Ok(val.borrow::<ExternalTool>()?.clone()),
            _ => Err(mlua::Error::RuntimeError(format!("Unable to convert value to action: {:?}", &value)))
        }
    }
}
