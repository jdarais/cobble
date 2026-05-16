// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::{
    borrow::Cow,
    collections::BTreeSet,
    path::{Component, Path, PathBuf, MAIN_SEPARATOR},
};

use glob::{glob, Pattern};

use mlua::{AnyUserData, Error, Lua, MultiValue, Table, UserData, Value};

pub struct PathLib;

impl UserData for PathLib {
    fn add_fields<F: mlua::prelude::LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_function_get("SEP", get_path_separator);
    }

    fn add_methods<M: mlua::prelude::LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("join", path_join);
        methods.add_function("glob", glob_files);
        methods.add_function("is_dir", is_dir);
        methods.add_function("is_file", is_file);
        methods.add_function("strip_prefix", strip_prefix);
    }
}

fn get_path_separator(_lua: &Lua, _: AnyUserData) -> mlua::Result<String> {
    Ok(String::from(MAIN_SEPARATOR))
}

fn path_join(_lua: &Lua, components: mlua::Variadic<String>) -> mlua::Result<String> {
    let mut path = PathBuf::new();
    for component in components {
        path.push(component.as_str());
    }

    match path.to_str() {
        Some(path_str) => Ok(path_str.to_owned()),
        None => Err(Error::runtime(format!(
            "Unable to convert path to a string: {}",
            path.display()
        ))),
    }
}

fn is_dir(_lua: &Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_dir())
}

fn is_file(_lua: &Lua, path_str: String) -> mlua::Result<bool> {
    Ok(Path::new(path_str.as_str()).is_file())
}

fn strip_prefix(
    _lua: &Lua,
    args: (String, String)
) -> mlua::Result<String> {
    let (path_str, prefix_str) = args;

    let path = Path::new(".").join(path_str.as_str());
    let prefix = Path::new(".").join(prefix_str.as_str());

    let stripped = path
        .strip_prefix(prefix.as_path())
        .map_err(|_| {
            Error::runtime(format!(
                "Cannot strip prefix. Path {} does not start with {}",
                path_str, prefix_str
            ))
        })?;

    stripped
        .to_str()
        .map(|s| s.to_owned())
        .ok_or_else(|| Error::runtime("Error converting path to string"))
}

fn normalize_path_for_glob(path: &Path) -> PathBuf {
    let mut norm_components: Vec<Component> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => { /* Ignore */ }
            _ => {
                norm_components.push(comp);
            }
        }
    }

    norm_components.into_iter().collect()
}

fn glob_files(lua: &Lua, args: MultiValue) -> mlua::Result<Table> {
    let arg_values: (String, Value, Value) = lua.unpack_multi(args)?;

    let (base_opt, path, options_opt) = match arg_values {
        (path, Value::Nil, Value::Nil) => Ok((None, path, None)),
        (path, Value::Table(opts), Value::Nil) => Ok((None, path, Some(opts))),
        (_, Value::Table(_), _) => Err(Error::runtime(
            "Unexpected argument after path (string) and options (table)",
        )),
        (base, Value::String(path), Value::Nil) => {
            Ok((Some(base), String::from(path.to_str()?.as_ref()), None))
        }
        (base, Value::String(path), Value::Table(opts)) => {
            Ok((Some(base), String::from(path.to_str()?.as_ref()), Some(opts)))
        }
        (_, Value::String(_), _) => Err(Error::runtime(
            "Expected options (table) or nil as third argument",
        )),
        (_, _, _) => Err(Error::runtime(
            "Expected path (string) or options (table) as second argument",
        )),
    }?;

    let mut include_patterns: Vec<String> = vec![path];
    let mut exclude_patterns: Vec<String> = Vec::new();
    let mut include_dirs = true;
    let mut include_files = true;

    if let Some(options) = options_opt {
        for pair in options.pairs() {
            let (k, v): (String, Value) = pair?;
            match k.as_str() {
                "include" => match v {
                    Value::String(s) => {
                        include_patterns.push(String::from(s.to_str()?.as_ref()));
                    }
                    Value::Table(t) => {
                        for incl_pair in t.clone().pairs() {
                            let (_, incl): (mlua::Value, String) = incl_pair?;
                            include_patterns.push(incl);
                        }
                    }
                    _ => {
                        return Err(Error::runtime(
                            "Expected string or list of strings for 'include' option",
                        ));
                    }
                },
                "exclude" => match v {
                    Value::String(s) => {
                        exclude_patterns.push(String::from(s.to_str()?.as_ref()));
                    }
                    Value::Table(t) => {
                        for excl_pair in t.clone().pairs() {
                            let (_, excl): (mlua::Value, String) = excl_pair?;
                            exclude_patterns.push(excl);
                        }
                    }
                    _ => {
                        return Err(Error::runtime(
                            "Expected string or list of strings for 'exclued' option",
                        ));
                    }
                },
                "include_dirs" => match v {
                    Value::Boolean(b) => {
                        include_dirs = b;
                    }
                    _ => {
                        return Err(Error::runtime(
                            "Expected boolean value for 'include_dirs' option",
                        ));
                    }
                },
                "include_files" => match v {
                    Value::Boolean(b) => {
                        include_files = b;
                    }
                    _ => {
                        return Err(Error::runtime(
                            "Expected boolean value for 'include_files' option",
                        ));
                    }
                },
                _ => {
                    return Err(Error::runtime(format!("Invalid option: {k}")));
                }
            }
        }
    }

    let base_path_opt = base_opt.map(|b| normalize_path_for_glob(&Path::new(b.as_str())));

    let mut include_glob_patterns: Vec<Cow<str>> = Vec::with_capacity(include_patterns.len());

    for incl in include_patterns.iter() {
        match base_path_opt.as_ref() {
            Some(base) => {
                if Path::new(incl.as_str()).is_absolute() {
                    return Err(Error::runtime(format!(
                        "If base path is provided, glob pattern must be relative. base={}, glob={}",
                        base.display(),
                        incl
                    )));
                }

                let pattern_path = base.join(incl);
                let pattern = pattern_path.to_str().ok_or_else(|| {
                    Error::runtime(format!(
                        "Error converting path to utf-8: {}",
                        pattern_path.display()
                    ))
                })?;
                include_glob_patterns.push(Cow::Owned(pattern.to_owned()));
            }
            None => {
                include_glob_patterns.push(Cow::Borrowed(incl.as_str()));
            }
        };
    }

    let mut exclude_glob_patterns: Vec<Pattern> = Vec::with_capacity(exclude_patterns.len());

    for excl in exclude_patterns.iter() {
        match base_path_opt.as_ref() {
            Some(base) => {
                if Path::new(excl.as_str()).is_absolute() {
                    return Err(Error::runtime(format!(
                        "If base path is provided, glob pattern must be relative. base={}, glob={}",
                        base.display(),
                        excl
                    )));
                }

                let pattern_path = base.join(excl);
                let pattern = pattern_path.to_str().ok_or_else(|| {
                    Error::runtime(format!(
                        "Error converting path to utf-8: {}",
                        pattern_path.display()
                    ))
                })?;
                let glob_pattern = Pattern::new(pattern)
                    .map_err(|e| Error::runtime(format!("Invalid glob pattern: {e}")))?;
                exclude_glob_patterns.push(glob_pattern);
            }
            None => {
                let glob_pattern = Pattern::new(excl.as_str())
                    .map_err(|e| Error::runtime(format!("Invalid glob pattern: {e}")))?;
                exclude_glob_patterns.push(glob_pattern);
            }
        };
    }

    let mut result_set: BTreeSet<String> = BTreeSet::new();

    for include_glob_pattern in include_glob_patterns.iter() {
        let glob_iter = glob(include_glob_pattern.as_ref())
            .map_err(|e| Error::runtime(format!("glob error: {}", e)))?;
        'entry: for entry_res in glob_iter {
            if let Ok(entry) = entry_res {
                let entry_path = entry.as_path();
                for exclude_glob_pattern in exclude_glob_patterns.iter() {
                    if exclude_glob_pattern.matches_path(entry_path) {
                        continue 'entry;
                    }
                }

                if entry_path.is_dir() && !include_dirs {
                    continue;
                }

                if entry_path.is_file() && !include_files {
                    continue;
                }

                let path_rel_to_base_res = match base_path_opt.as_ref() {
                    Some(base) => entry.strip_prefix(base.as_path()),
                    None => Ok(entry_path),
                };

                if let Ok(path_rel_to_base) = path_rel_to_base_res {
                    if let Some(path_str) = path_rel_to_base.to_str() {
                        result_set.insert(String::from(path_str));
                    }
                }
            }
        }
    }

    let result_table = lua.create_table()?;
    for (i, result) in result_set.into_iter().enumerate() {
        result_table.set(i + 1, result)?;
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
            format!("four{sep}five{sep}six{sep}foo.txt"),
        ];
        assert_eq!(files.len().unwrap() as usize, expected_paths.len());
        for val_res in files.sequence_values() {
            let val: String = val_res.unwrap();
            assert!(
                expected_paths.contains(&val),
                "Unexpected result path {}",
                val
            );
        }
    }
}
