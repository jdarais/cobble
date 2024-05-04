extern crate mlua;
extern crate toml;
extern crate serde;

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::datamodel::{
    Action, ActionCmd, BuildEnv, ExternalTool, Project, TaskDef
};
use crate::lua::detached_value::dump_function;
use crate::lua::lua_env::create_lua_env;
use crate::workspace::resolve::resolve_names_in_project;
use crate::workspace::config::PROJECT_FILE_NAME;


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
        dir.to_owned()
    };

    let project_file_path = workspace_dir.join(project_dir.as_path()).join(PROJECT_FILE_NAME);
    if !project_file_path.exists() {
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

    let create_task_func = lua.create_function(|_, task: TaskDef| {
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
                dir = dir or WORKSPACE.dir
            end

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
        let (key, mut value): (String, Project) = pair?;
        let resolved_project_res = resolve_names_in_project(&mut value);
        match resolved_project_res {
            Ok(_) => { projects.insert(key, value); },
            Err(e) => { return Err(mlua::Error::runtime(format!("{}", e))); }
        }
    }

    //
    // Inject an __COBBLE_INTERNAL__ project with the "cmd" tool
    //
    let cmd_tool_action_func: mlua::Function = lua.load(r#"
        function (c)
            local result = cmd { cwd = c.project.dir, table.unpack(c.args) }

            c.out(result.stdout)
            c.err(result.stderr)
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
            cmd: ActionCmd::Func(dump_function(cmd_tool_action_func, lua, &HashSet::new())?)
        }
    };

    projects.insert(String::from("/__COBBLE_INTERNAL__"), Project {
        name: Arc::<str>::from("/__COBBLE_INTERNAL__"),
        path: PathBuf::from("./__COBBLE_INTERNAL__").into(),
        build_envs: Vec::new(),
        tasks: Vec::new(),
        tools: vec![cmd_tool],
        child_project_names: Vec::new()
    });
    // End __COBBLE_INTERNAL__ project

    Ok(projects)
}

pub fn load_projects<'a, P>(workspace_dir: &Path, root_projects: P) -> mlua::Result<HashMap<String, Project>>
    where P: Iterator<Item = &'a str>
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
        assert_eq!(projects.len(), 2);

        let project = projects.get("/testproject").unwrap();
        assert_eq!(project.build_envs.len(), 1);
        assert_eq!(project.tasks.len(), 0);
        assert_eq!(project.path.as_ref(), Path::new("testproject"))
    }
}