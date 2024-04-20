extern crate mlua;
extern crate toml;
extern crate serde;

use std::collections::HashMap;
use std::io;
use std::env;
use std::fmt::Display;
use std::fs::File;
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Serialize, Deserialize};

use crate::cobble::datamodel::{
    BuildEnv,
    Task,
    ExternalTool
};

pub const WORKSPACE_CONFIG_FILE_NAME: &str = "cobble.toml";

#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub build_envs: Vec<BuildEnv>,
    pub tasks: Vec<Task>,
    pub tools: Vec<ExternalTool>
}

impl Display for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Project(")?;
        write!(f, "name=\"{}\",", &self.name)?;
        write!(f, "path={},", self.path.display())?;
        
        f.write_str("build_envs=[")?;
        for (i, build_env) in self.build_envs.iter().enumerate() {
            if i > 0 { f.write_str(", ")? }
            write!(f, "{}", build_env)?;
        }
        f.write_str("], ")?;

        f.write_str("tasks=[")?;
        for (i, task) in self.tasks.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", task)?;
        }
        f.write_str("], ")?;

        f.write_str("tools=[")?;
        for (i, tool) in self.tools.iter().enumerate() {
            if i > 0 { f.write_str(", ")?; }
            write!(f, "{}", tool)?;
        }
        f.write_str("]")?;

        f.write_str(")")
    }
}

impl <'lua> mlua::FromLua<'lua> for Project {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let project_table = match value {
            mlua::Value::Table(tbl) => tbl,
            _ => { return Err(mlua::Error::RuntimeError(format!("Project must be a lua table value"))); }
        };

        let name: String = project_table.get("name")?;
        let path_str: String = project_table.get("dir")?;
        let path = PathBuf::from_str(path_str.as_str()).expect("Conversion from str to PathBuf is infalliable");
        let build_envs: Vec<BuildEnv> = project_table.get("build_envs")?;
        let tasks: Vec<Task> = project_table.get("tasks")?;
        let tools: Vec<ExternalTool> = project_table.get("tools")?;

        Ok(Project{ name, path, build_envs, tasks, tools })
    }
}

pub struct WorkspaceConfig {
    pub root_projects: Vec<String>
}

pub struct WorkspaceDef {
    pub projects: HashMap<String, Project>
}

#[derive(Debug)]
pub enum WorkspaceConfigError {
    Unknown,
    FileError{path: PathBuf, error: io::Error},
    ParseError(String),
    ValueError(String)
}

impl Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use WorkspaceConfigError::*;
        match self {
            Unknown => write!(f, "Unknown Error"),
            FileError{path, error} => write!(f, "Error reading file at {}: {}", path.display(), error),
            ParseError(msg) => write!(f, "Error parsing config file: {}", msg),
            ValueError(msg) => write!(f, "Error reading config values: {}", msg)
        }
    }
}

pub fn parse_workspace_config(mut config_str: &str) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config: toml::Table = config_str.parse().map_err(|e| WorkspaceConfigError::ParseError(format!("Error parsing config: {}", e)))?;

    let root_projects_opt: Option<toml::Value> = config.remove("root_projects");
    let root_projects: Vec<String> = match root_projects_opt {
        None => vec![String::from(".")],
        Some(val) => val.try_into()
            .map_err(|e| WorkspaceConfigError::ValueError(format!("at 'root_projects': {}", e)))?
    };

    // Raise an error if there are unrecognized keys in the config table
    if let Some((key, _)) = config.iter().next() {
        return Err(WorkspaceConfigError::ValueError(format!("Unrecognized field '{}'", key)));
    }

    Ok(WorkspaceConfig{
        root_projects
    })
}

pub fn parse_workspace_config_file(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config_file = File::open(path).map_err(|e| WorkspaceConfigError::FileError{path: PathBuf::from(path), error: e})?;

    let mut config_toml_str = String::new();
    let file_read_res = config_file.read_to_string(&mut config_toml_str);
    if let Err(e) = file_read_res {
        return Err(WorkspaceConfigError::FileError{path: PathBuf::from(path), error: e});
    }

    parse_workspace_config(config_toml_str.as_str())
}

pub struct Workspace {
    workspace_dir: PathBuf,
    config: WorkspaceConfig
}

impl Workspace {
    pub fn new(dir: &Path, config: WorkspaceConfig) -> Workspace {
        Workspace {
            workspace_dir: PathBuf::from(dir),
            config: config
        }
    }
}

pub fn find_nearest_workspace_dir_from(path: &Path) -> Result<PathBuf, io::Error> {
    for ancestor in path.canonicalize()?.ancestors() {
       if ancestor.join(WORKSPACE_CONFIG_FILE_NAME).exists() {
        return Ok(PathBuf::from(ancestor));
       } 
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Did not find '{}' file in any ancestor directory from {}", WORKSPACE_CONFIG_FILE_NAME, path.display()))
    )
}

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
        return Err(mlua::Error::RuntimeError(format!("Project file {} doesn't exist", project_file_path.display())));
    }

    let mut project_file = match File::open(&project_file_path) {
        Ok(f) => f,
        Err(e) => {
            return Err(mlua::Error::RuntimeError(format!("Unable to open file {}: {}", project_file_path.display(), e)));
        }
    };

    let mut project_source = String::new();
    let project_file_read_res = project_file.read_to_string(&mut project_source);
    if let Err(e) = project_file_read_res {
        return Err(mlua::Error::RuntimeError(format!("Error reading fiel {}: {}", project_file_path.display(), e)));
    }

    let project_dir_str = project_dir.as_os_str().to_str()
        .ok_or_else(|| mlua::Error::RuntimeError(String::from("Unable to convert project path to string")))?;
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
        projects.insert(key, value);
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_parse_workspace_config() {
        let config_toml = r#"
            root_projects = ["proj1", "proj2", "proj3"]
        "#;

        let config = parse_workspace_config(config_toml).unwrap();
        assert_eq!(config.root_projects, vec!["proj1", "proj2", "proj3"]);
    }

    #[test]
    fn test_init_lua() {
        let workspace = Workspace::new(
            Path::new("test"),
            WorkspaceConfig {
                root_projects: vec![String::from(".")]
            }
        );
        let lua = create_lua_env(Path::new(".")).unwrap();

        init_lua_for_project_config(&lua, &workspace.workspace_dir.as_path()).unwrap();

        let ws_dir: String = lua.load(r#"WORKSPACE.dir"#).eval().unwrap();
        assert_eq!(ws_dir, "test");
    }

    #[test]
    fn test_load_subproject_def() {
        let lua = create_lua_env(Path::new(".")).unwrap();

        init_lua_for_project_config(&lua, Path::new("testproject")).unwrap();

        process_project(
            &lua,
            r#"
            build_env({
                name = "test",
                install_actions = {
                    function () print("hi!") end
                },
                install_deps = {},
                exec = function (a) print(a) end
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