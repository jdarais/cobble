use std::fmt;

#[derive(Clone, Debug)]
pub struct Artifact {
    pub filename: String
}

impl fmt::Display for Artifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.filename.as_str())
    }
}

impl <'lua> mlua::FromLua<'lua> for Artifact {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let filename: String = lua.unpack(value)?;

        Ok(Artifact{ filename })
    }
}
