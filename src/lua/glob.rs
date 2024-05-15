// TODO

use std::path::{Path, MAIN_SEPARATOR};

use glob::glob;

use mlua::{Error, Lua, MultiValue, Table};

pub fn glob_files<'lua>(lua: &'lua Lua, args: MultiValue<'lua>) -> mlua::Result<Table<'lua>> {
    let (path_or_base, mut path_opt): (String, Option<String>) = lua.unpack_multi(args)?;
    let mut path_or_base_opt = Some(path_or_base);
    let path = path_opt.take().or_else(|| path_or_base_opt.take()).unwrap();
    let base_opt = path_or_base_opt.take();

    let glob_pattern = match base_opt.as_ref() {
        Some(base) => {
            if Path::new(path.as_str()).is_absolute() {
                return Err(Error::runtime(format!(
                    "If base path is provided, glob pattern must be relative. base={}, glob={}",
                    base, path
                )));
            }

            let mut pattern = base.clone();
            pattern.push(MAIN_SEPARATOR);
            pattern.push_str(path.as_str());
            pattern
        }
        None => path.clone(),
    };

    let result_table = lua.create_table()?;

    let glob_iter = glob(glob_pattern.as_str()).map_err(|e| Error::runtime(format!("glob error: {}", e)))?;
    for entry_res in glob_iter {
        if let Ok(entry) = entry_res {
            let path_rel_to_base_res = match base_opt.as_ref() {
                Some(base) => entry.strip_prefix(Path::new(base.as_str())),
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
        let expected_paths = vec![
            "one/two/three/foo.txt",
            "one/two/foo.txt",
            "four/five/six/foo.txt"
        ];
        assert_eq!(files.len().unwrap() as usize, expected_paths.len());
        for val_res in files.sequence_values() {
            let val: String = val_res.unwrap();
            assert!(expected_paths.contains(&val.as_str()), "Unexpected result path {}", val);
        }
    }
}

