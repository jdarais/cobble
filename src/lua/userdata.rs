// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::lua::fs::FsLib;
use crate::lua::json::JsonLib;
use crate::lua::toml::TomlLib;
use crate::lua::{cmd::CmdLib, path::PathLib, script_dir::ScriptDirLib};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CobbleUserData {
    FsLib,
    PathLib,
    CmdLib,
    ScriptDirLib,
    TomlLib,
    JsonLib,
}

impl CobbleUserData {
    pub fn to_userdata(&self, lua: &mlua::Lua) -> mlua::Result<mlua::AnyUserData> {
        match self {
            CobbleUserData::FsLib => lua.create_userdata(FsLib),
            CobbleUserData::PathLib => lua.create_userdata(PathLib),
            CobbleUserData::CmdLib => lua.create_userdata(CmdLib),
            CobbleUserData::ScriptDirLib => lua.create_userdata(ScriptDirLib),
            CobbleUserData::TomlLib => lua.create_userdata(TomlLib),
            CobbleUserData::JsonLib => lua.create_userdata(JsonLib),
        }
    }

    pub fn from_userdata(
        _lua: &mlua::Lua,
        userdata: mlua::AnyUserData,
    ) -> mlua::Result<CobbleUserData> {
        if mlua::AnyUserData::is::<FsLib>(&userdata) {
            return Ok(CobbleUserData::FsLib);
        } else if mlua::AnyUserData::is::<PathLib>(&userdata) {
            return Ok(CobbleUserData::PathLib);
        } else if mlua::AnyUserData::is::<CmdLib>(&userdata) {
            return Ok(CobbleUserData::CmdLib);
        } else if mlua::AnyUserData::is::<ScriptDirLib>(&userdata) {
            return Ok(CobbleUserData::ScriptDirLib);
        } else if mlua::AnyUserData::is::<TomlLib>(&userdata) {
            return Ok(CobbleUserData::TomlLib);
        } else if mlua::AnyUserData::is::<JsonLib>(&userdata) {
            return Ok(CobbleUserData::JsonLib);
        } else {
            return Err(mlua::Error::runtime("Unable to serialize userdata value"));
        }
    }
}

impl fmt::Display for CobbleUserData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CobbleUserData::*;
        match self {
            FsLib => write!(f, "FsLib"),
            PathLib => write!(f, "PathLib"),
            CmdLib => write!(f, "CmdLib"),
            ScriptDirLib => write!(f, "ScriptDirLib"),
            TomlLib => write!(f, "TomlLib"),
            JsonLib => write!(f, "JsonLib"),
        }
    }
}

impl Hash for CobbleUserData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}
