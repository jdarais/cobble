extern crate mlua;
extern crate toml;
extern crate serde;

use std::collections::HashMap;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::datamodel::{
    BuildEnv,
    Task,
    ExternalTool,
    Project
};
use crate::workspace::resolve::resolve_names_in_project;


fn process_project(lua: &mlua::Lua, project_source: &str, project_name: &str, project_dir: &str) -> mlua::Result<()> {
    let start_project: mlua::Function = lua.globals().get("start_project")?;
    let end_project: mlua::Function = lua.globals().get("end_project")?;

    start_project.call((project_name, project_dir))?;

    lua.load(project_source).exec()?;

    end_project.call(())?;

    Ok(())
}

pub fn process_project_file(lua: &mlua::Lua, dir: &str, workspace_dir: &Path) -> mlua::Result<()> {
    let current_project: Option<mlua::Table> = lua.globals().get("PROJECT")?;

    let project_dir = match current_project.as_ref() {
        Some(cur_proj) => {
            let current_project_dir: String = cur_proj.get("dir")?;
            Path::new(current_project_dir.as_str()).join(&dir)
        },
        None => PathBuf::from(dir)
    };

    let project_name = if dir == "" || dir == "." {
        String::new()
    } else {
        String::from(dir)
    };

    let project_file_path = project_dir.join("project.lua");
    if !workspace_dir.join(&project_file_path).exists() {
        return Err(mlua::Error::runtime(format!("Project file {} doesn't exist", project_file_path.display())));
    }

    let mut project_file = match File::open(&project_file_path) {
        Ok(f) => f,
        Err(e) => {
            return Err(mlua::Error::runtime(format!("Unable to open file {}: {}", project_file_path.display(), e)));
        }
    };

    let mut project_source = String::new();
    let project_file_read_res = project_file.read_to_string(&mut project_source);
    if let Err(e) = project_file_read_res {
        return Err(mlua::Error::runtime(format!("Error reading fiel {}: {}", project_file_path.display(), e)));
    }

    let project_dir_str = project_dir.as_os_str().to_str()
        .ok_or_else(|| mlua::Error::runtime("Unable to convert project path to string"))?;
    process_project(lua, project_source.as_str(), project_name.as_str(), project_dir_str)?;

    Ok(())
}

pub fn init_lua_for_project_config(lua: &mlua::Lua, workspace_dir: &Path) -> mlua::Result<()> {
    let cxt = lua.create_table()?;
    cxt.set("ws_dir", workspace_dir.to_str().unwrap_or("."))?;
    
    let workspace_dir_owned = PathBuf::from(workspace_dir);
    let project_dir_func = lua.create_function(move |lua, dir: String| {
        process_project_file(lua, dir.as_str(), workspace_dir_owned.as_path())
    })?;
    cxt.set("process_project_dir", project_dir_func)?;

    let create_build_env = lua.create_function(|_, build_env: BuildEnv| {
        Ok(mlua::AnyUserData::wrap(build_env))
    })?;
    cxt.set("create_build_env", create_build_env)?;

    let create_task_func = lua.create_function(|_, task: Task| {
        Ok(mlua::AnyUserData::wrap(task))
    })?;
    cxt.set("create_task", create_task_func)?;

    let create_tool_func = lua.create_function(|_, tool: ExternalTool| {
        Ok(mlua::AnyUserData::wrap(tool))
    })?;
    cxt.set("create_external_tool", create_tool_func)?;

    lua.load(r#"
        local cxt = ...

        cobble = {
            workspace = {
                dir = cxt.ws_dir
            },
            projects = {},
        }

        PROJECT = nil
        WORKSPACE = cobble.workspace

        _project_stack = {}

        function start_project (name, dir)
            if PROJECT then
                if name == "" then
                    error("Empty name is only allowed for the root project!")
                end

                if PROJECT.name == "/" then
                    name = "/" .. name
                else
                    name = PROJECT.name .. "/" .. name
                end
                dir = dir or PROJECT.dir
            else
                name = "/" .. (name or "")
            end

            dir = dir or WORKSPACE.dir

            if cobble.projects[name] then
                error("Project " .. name .. " already exists!")
            end

            local project = {
                name = name,
                dir = dir,
                build_envs = {},
                tasks = {},
                tools = {},
                child_projects = {}
            }
            
            if PROJECT then table.insert(PROJECT.child_projects, project) end

            cobble.projects[name] = project
            table.insert(_project_stack, project)
            PROJECT = project
        end

        function end_project ()
            table.remove(_project_stack)
            PROJECT = _project_stack[#_project_stack]
        end

        function project (name, def_func)
            start_project(name)
            def_func()
            end_project()
        end

        function project_dir (dir)
            cxt.process_project_dir(dir)
        end

        function build_env (env)
            local created = cxt.create_build_env(env)
            table.insert(PROJECT.build_envs, created)
        end

        function external_tool (tool)
            local created = cxt.create_external_tool(tool)
            table.insert(PROJECT.tools, created)
        end

        function task (tsk)
            local created = cxt.create_task(tsk)
            table.insert(PROJECT.tasks, created)
        end
    "#).call(cxt)
}

pub fn extract_project_defs(lua: &mlua::Lua) -> mlua::Result<HashMap<String, Project>> {
    let cobble_table: mlua::Table = lua.globals().get("cobble")?;
    let projects_table: mlua::Table = cobble_table.get("projects")?;

    let mut projects: HashMap<String, Project> = HashMap::new();

    for pair in projects_table.pairs() {
        let (key, value): (String, Project) = pair?;
        let resolved_project_res = resolve_names_in_project(&value);
        match resolved_project_res {
            Ok(resolved_project) => { projects.insert(key, resolved_project); },
            Err(e) => { return Err(mlua::Error::runtime(format!("{}", e))); }
        }
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua::lua_env::create_lua_env;


    #[test]
    fn test_load_subproject_def() {
        let lua = create_lua_env(Path::new(".")).unwrap();

        init_lua_for_project_config(&lua, Path::new("testproject")).unwrap();

        process_project(
            &lua,
            r#"
            build_env({
                name = "test",
                install = {
                    function () print("hi!") end
                },
                deps = {},
                action = function (a) print(a) end
            })
        "#,
    "testproject", 
    "testproject"
        ).unwrap();

        let projects = extract_project_defs(&lua).unwrap();
        assert_eq!(projects.len(), 1);

        let project = projects.values().next().unwrap();
        assert_eq!(project.build_envs.len(), 1);
        assert_eq!(project.tasks.len(), 0);
        assert_eq!(project.path, Path::new("testproject"))
    }
}