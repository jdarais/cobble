// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::io::ErrorKind;

use mlua::UserData;



pub struct FsLib;

impl UserData for FsLib {
    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("mkdir", mkdir);
        methods.add_function("rmdir", rmdir);
        methods.add_function("copy", copy_file);
        methods.add_function("remove", remove_file);
    }
}

fn mkdir<'lua>(lua: &'lua mlua::Lua, args: (String, Option<mlua::Table>)) -> mlua::Result<()> {
    let (dir_path, opts_opt) = args;
    
    let mut parents = false;
    let mut allow_existing = false;

    if let Some(opts) = opts_opt {
        for opt in opts.pairs() {
            let (k, v): (String, mlua::Value) = opt?;
            match k.as_str() {
                "parents" => { parents = lua.unpack(v)?; }
                "allow_existing" => { allow_existing = lua.unpack(v)?; }
                _ => { return Err(mlua::Error::runtime(format!("Unknown option: {k}"))); }
            }
        }
    }

    let create_dir_res = match parents {
        true => std::fs::create_dir_all(&dir_path),
        false => std::fs::create_dir(&dir_path),
    };

    if let Err(e) = create_dir_res {
        if e.kind() == ErrorKind::AlreadyExists && allow_existing {
            return Ok(())
        } else {
            return Err(mlua::Error::runtime(format!("Error creating directory {dir_path}: {e}")));
        }
    }

    Ok(())
}


fn rmdir<'lua>(lua: &'lua mlua::Lua, args: (String, Option<mlua::Table>)) -> mlua::Result<()> {
    let (dir_path, opts_opt) = args;

    let mut ignore_not_found = false;
    let mut recursive = false;

    if let Some(opts) = opts_opt {
        for opt in opts.pairs() {
            let (k, v): (String, mlua::Value) = opt?;
            match k.as_str() {
                "ignore_not_found" => { ignore_not_found = lua.unpack(v)?; }
                "recursive" => { recursive = lua.unpack(v)?; }
                _ => { return Err(mlua::Error::runtime(format!("Unknown option: {k}"))); }
            }
        }
    }

    let remove_dir_res = match recursive {
        true => std::fs::remove_dir_all(&dir_path),
        false => std::fs::remove_dir(&dir_path),
    };

    if let Err(e) = remove_dir_res {
        if e.kind() == ErrorKind::NotFound && ignore_not_found {
            return Ok(())
        } else {
            return Err(mlua::Error::runtime(format!("Error removing directory {dir_path}: {e}")))
        }
    }

    Ok(())
}

fn copy_file<'lua>(_lua: &'lua mlua::Lua, args: (String, String)) -> mlua::Result<()> {
    let (from_path, to_path) = args;
    std::fs::copy(&from_path, &to_path).map(|_| ()).map_err(|e| mlua::Error::runtime(format!("Failed to copy file {from_path} to {to_path}: {e}")))
}

fn remove_file<'lua>(_lua: &'lua mlua::Lua, path: String) -> mlua::Result<()> {
    std::fs::remove_file(&path).map_err(|e| mlua::Error::runtime(format!("Failed to remove file {path}: {e}")))
}
