use std::collections::HashMap;
use std::env::{current_dir, set_current_dir};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::PROJECT_FILE_NAME;
use crate::lua::lua_env::create_lua_env;
use crate::lua::detached::dump_function;
use crate::project_def::build_env::validate_build_env;
use crate::project_def::task::validate_task;
use crate::project_def::tool::validate_tool;
use crate::project_def::{Action, ActionCmd, ExternalTool, Project};
use crate::resolve::resolve_names_in_project;
use crate::util::onscopeexit::OnScopeExit;

fn process_project(
    lua: &mlua::Lua,
    chunk: &Path,
    project_name: &str,
    workspace_dir: &Path,
    project_dir: &str
) -> mlua::Result<()> {
    let start_project: mlua::Function = lua.globals().get("start_project")?;
    let end_project: mlua::Function = lua.globals().get("end_project")?;

    let prev_cwd = current_dir().map_err(|e| {
        mlua::Error::runtime(format!("Unable to get current working directory: {}", e))
    })?;

    set_current_dir(&workspace_dir.join(project_dir)).map_err(|e| {
        mlua::Error::runtime(format!(
            "Unable to set the current working directory to {}: {}",
            project_dir, e
        ))
    })?;

    let _restore_cwd = OnScopeExit::new(Box::new(|| {
        set_current_dir(prev_cwd)
            .expect("expected to be able to set current working directory to previous value");
    }));

    start_project.call((project_name, project_dir))?;

    lua.load(chunk).exec()?;

    end_project.call(())?;

    Ok(())
}

pub fn process_project_file(lua: &mlua::Lua, dir: &str, workspace_dir: &Path) -> mlua::Result<()> {
    let current_project: Option<mlua::Table> = lua.globals().get("PROJECT")?;

    let project_dir = match current_project.as_ref() {
        Some(cur_proj) => {
            let current_project_dir: String = cur_proj.get("dir")?;
            PathBuf::from_iter(
                Path::new(current_project_dir.as_str())
                    .join(&dir)
                    .components(),
            )
        }
        None => PathBuf::from(dir),
    };

    let project_name = if dir == "" || dir == "." {
        String::new()
    } else {
        dir.to_owned()
    };

    let project_file_path = PathBuf::from_iter(
        workspace_dir
            .join(project_dir.as_path())
            .join(PROJECT_FILE_NAME)
            .components(),
    );
    if !project_file_path.exists() {
        return Err(mlua::Error::runtime(format!(
            "Project file {} doesn't exist",
            project_file_path.display()
        )));
    }

    let project_dir_str = project_dir
        .as_os_str()
        .to_str()
        .ok_or_else(|| mlua::Error::runtime("Unable to convert project path to string"))?;
    process_project(
        lua,
        project_file_path.as_path(),
        project_name.as_str(),
        workspace_dir,
        project_dir_str,
    )?;

    Ok(())
}

pub fn init_lua_for_project_config(lua: &mlua::Lua, workspace_dir: &Path) -> mlua::Result<()> {
    let cxt = lua.create_table()?;
    cxt.set("ws_dir", workspace_dir.to_str().unwrap_or("."))?;

    let strip_path_prefix_func =
        lua.create_function(|_lua, (path, prefix): (String, String)| {
            let stripped_path = Path::new(path.as_str()).strip_prefix(Path::new(prefix.as_str()));
            match stripped_path {
                Ok(p) => Ok(p.to_str().map(|s| s.to_owned()).unwrap_or(path)),
                Err(_) => Ok(path),
            }
        })?;
    cxt.set("strip_path_prefix", strip_path_prefix_func)?;

    let workspace_dir_owned = PathBuf::from(workspace_dir);
    let project_dir_func = lua.create_function(move |lua, dir: String| {
        process_project_file(lua, dir.as_str(), workspace_dir_owned.as_path())
    })?;
    cxt.set("process_project_dir", project_dir_func)?;

    let validate_build_env =
        lua.create_function(|lua, val: mlua::Value| validate_build_env(lua, &val))?;
    cxt.set("validate_build_env", validate_build_env)?;

    let validate_task = lua.create_function(|lua, val: mlua::Value| validate_task(lua, &val))?;
    cxt.set("validate_task", validate_task)?;

    let validate_tool = lua.create_function(|lua, val: mlua::Value| validate_tool(lua, &val))?;
    cxt.set("validate_tool", validate_tool)?;

    cxt.set("project_file_name", PROJECT_FILE_NAME)?;

    let project_config_source = include_bytes!("project_config.lua");
    lua.load(&project_config_source[..]).call(cxt)
}

pub fn extract_project_defs(lua: &mlua::Lua) -> mlua::Result<HashMap<String, Project>> {
    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let projects_table: mlua::Table = cobble_table.get("projects")?;

    let mut projects: HashMap<String, Project> = HashMap::new();

    for pair in projects_table.pairs() {
        let (key, mut value): (String, Project) = pair?;
        let resolved_project_res = resolve_names_in_project(&mut value);
        match resolved_project_res {
            Ok(_) => {
                projects.insert(key, value);
            }
            Err(e) => {
                return Err(mlua::Error::runtime(format!("{}", e)));
            }
        }
    }

    //
    // Inject an __COBBLE_INTERNAL__ project with the "cmd" tool
    //
    let cmd_tool_action_func: mlua::Function = lua.load(r#"
        function (c)
            local cmd = require("cmd")
            local result = cmd { cwd = c.project.dir, out = c.out, err = c.err, table.unpack(c.args) }

            if result.status ~= 0 then
                error("Command '" .. table.concat(c.args, " ") .. "' exited with status " .. result.status, 0)
            end

            return result
        end
    "#).eval()?;

    let cmd_tool = ExternalTool {
        name: Arc::<str>::from("cmd"),
        install: None,
        check: None,
        action: Action {
            tools: HashMap::new(),
            build_envs: HashMap::new(),
            cmd: ActionCmd::Func(dump_function(lua, cmd_tool_action_func, &mut HashMap::new())?),
        },
    };

    projects.insert(
        String::from("/__COBBLE_INTERNAL__"),
        Project {
            name: Arc::<str>::from("/__COBBLE_INTERNAL__"),
            path: PathBuf::from("./__COBBLE_INTERNAL__").into(),
            build_envs: Vec::new(),
            tasks: Vec::new(),
            tools: vec![cmd_tool],
            child_project_names: Vec::new(),
            project_source_deps: Vec::new(),
        },
    );
    // End __COBBLE_INTERNAL__ project

    Ok(projects)
}

pub fn load_projects<'a, P>(
    workspace_dir: &Path,
    root_projects: P,
) -> mlua::Result<HashMap<String, Project>>
where
    P: Iterator<Item = &'a str>,
{
    let project_def_lua = create_lua_env(workspace_dir)?;

    init_lua_for_project_config(&project_def_lua, workspace_dir)?;

    for project_dir in root_projects {
        process_project_file(&project_def_lua, project_dir, workspace_dir)?;
    }

    extract_project_defs(&project_def_lua)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::lua_env::create_lua_env;
    use std::{fs::File, io::Write};

    #[test]
    fn test_load_subproject_def() {
        let tmpdir = mktemp::Temp::new_dir().unwrap();
        let lua = create_lua_env(tmpdir.as_path()).unwrap();

        init_lua_for_project_config(&lua, tmpdir.as_path()).unwrap();

        let temp_proj_file_path = tmpdir.as_path().join("project.lua");
        {
            let mut f = File::create(&temp_proj_file_path).unwrap();
            f.write_all(
                br#"
                env({
                    name = "test",
                    install = {
                        function () print("hi!") end
                    },
                    deps = {},
                    action = function (a) print(a) end
                })
            "#,
            )
            .unwrap();
            f.flush().unwrap();
        }

        process_project(&lua, temp_proj_file_path.as_path(), "", &tmpdir, ".").unwrap();

        let projects = extract_project_defs(&lua).unwrap();
        assert_eq!(projects.len(), 2);

        let project = projects.get("/").unwrap();
        assert_eq!(project.build_envs.len(), 1);
        assert_eq!(project.tasks.len(), 0);
        assert_eq!(project.path.as_ref(), Path::new("."))
    }
}
