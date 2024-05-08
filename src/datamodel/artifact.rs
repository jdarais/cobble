use std::{borrow::Cow, fmt, sync::Arc};

use crate::datamodel::validate::validate_is_string;

#[derive(Clone, Debug)]
pub struct Artifact {
    pub filename: Arc<str>
}

pub fn validate_artifact<'lua>(_lua: &'lua mlua::Lua, value: &mlua::Value<'lua>, prop_name: Option<Cow<'static, str>>, prop_path: &mut Vec<Cow<'static, str>>) -> mlua::Result<()> {
    validate_is_string(value, prop_name, prop_path).and(Ok(()))
}

impl fmt::Display for Artifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.filename.as_ref())
    }
}

impl <'lua> mlua::FromLua<'lua> for Artifact {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let filename_str: String = lua.unpack(value)?;
        let filename = Arc::<str>::from(filename_str);

        Ok(Artifact{ filename })
    }
}
