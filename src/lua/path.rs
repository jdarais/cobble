// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};

use glob::glob;

use mlua::{AnyUserData, Error, Lua, MultiValue, Table, UserData};

pub struct FsLib;

impl UserData for FsLib {
    fn add_fields<'lua, F: mlua::prelude::LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_function_get("SEP", get_path_separator);
    }

    fn add_methods<'lua, M: mlua::prelude::LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("join", path_join);
        methods.add_function("glob", glob_files);
        methods.add_function("is_dir", is_dir);
        methods.add_function("is_file", is_file);
    }
}

fn get_path_separator<'lua>(_lua: &'lua Lua, _: AnyUserData<'lua>) -> mlua::Result<String> {
    Ok(String::from(MAIN_SEPARATOR))
}

fn path_join<'lua>(_lua: &'lua Lua, components: mlua::Variadic<String>) -> mlua::Result<String> {
    let mut path = PathBuf::new();
    for component in components {
        path.push(component.as_str());
    }

    match path.to_str() {
        Some(path_str) => Ok(path_str.to_owned()),
        None => Err(mlua::Error::runtime(format!("Unable to convert path to a string: {}", path.display())))
    }
}

fn is_dir<'lua>(_lua: &'lua Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_dir())
}

fn is_file<'lua>(_lua: &'lua Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_file())
}

fn normalize_path_for_glob(path: &Path) -> PathBuf {
    let mut norm_components: Vec<Component> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => { /* Ignore */ }
            _ => { norm_components.push(comp); }
        }
    }

    norm_components.into_iter().collect()
}

fn glob_files<'lua>(lua: &'lua Lua, args: MultiValue<'lua>) -> mlua::Result<Table<'lua>> {
    let (path_or_base, mut path_opt): (String, Option<String>) = lua.unpack_multi(args)?;
    let mut path_or_base_opt = Some(path_or_base);
    let path = path_opt.take().or_else(|| path_or_base_opt.take()).unwrap();
    let base_opt = path_or_base_opt.take().map(|s| normalize_path_for_glob(&Path::new(s.as_str())));

    let glob_pattern = match base_opt.as_ref() {
        Some(base) => {
            if Path::new(path.as_str()).is_absolute() {
                return Err(Error::runtime(format!(
                    "If base path is provided, glob pattern must be relative. base={}, glob={}",
                    base.display(), path
                )));
            }

            let pattern_path = base.join(path);
            let pattern = pattern_path.to_str().ok_or_else(|| mlua::Error::runtime(format!("Error converting path to utf-8: {}", pattern_path.display())))?;
            pattern.to_owned()
        }
        None => path.clone(),
    };

    let result_table = lua.create_table()?;

    let glob_iter = glob(glob_pattern.as_str()).map_err(|e| Error::runtime(format!("glob error: {}", e)))?;
    for entry_res in glob_iter {
        if let Ok(entry) = entry_res {
            let path_rel_to_base_res = match base_opt.as_ref() {
                Some(base) => entry.strip_prefix(base.as_path()),
                None => Ok(entry.as_path())
            };
            
            if let Ok(path_rel_to_base) = path_rel_to_base_res {
                if let Some(path_str) = path_rel_to_base.to_str() {
                    result_table.push(path_str)?;
                }
            }
        }
    }

    Ok(result_table)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use std::fs::{create_dir_all, File};

    use mktemp::Temp;

    #[test]
    fn test_glob() {
        let tempdir = Temp::new_dir().unwrap();
        create_dir_all(tempdir.join("one/two/three")).unwrap();
        create_dir_all(tempdir.join("four/five/six")).unwrap();
        File::create(tempdir.join("one/two/three/foo.txt")).unwrap();
        File::create(tempdir.join("one/two/foo.txt")).unwrap();
        File::create(tempdir.join("four/five/six/foo.txt")).unwrap();

        let lua = Lua::new();
        let base = tempdir.to_str().unwrap();
        let pattern = "**/foo.txt";
        let files = glob_files(&lua, lua.pack_multi((base, pattern)).unwrap()).unwrap();
        let sep = std::path::MAIN_SEPARATOR;
        let expected_paths = vec![
            format!("one{sep}two{sep}three{sep}foo.txt"),
            format!("one{sep}two{sep}foo.txt"),
            format!("four{sep}five{sep}six{sep}foo.txt")
        ];
        assert_eq!(files.len().unwrap() as usize, expected_paths.len());
        for val_res in files.sequence_values() {
            let val: String = val_res.unwrap();
            assert!(expected_paths.contains(&val), "Unexpected result path {}", val);
        }
    }
}

