extern crate mlua;

use std::fs::File;
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::path::{Path, PathBuf};

use crate::cobble::datamodel::{BuildEnv, Task};

pub struct Project {
    pub path: PathBuf,
    pub build_envs: Vec<BuildEnv>,
    pub tasks: Vec<Task>,
    pub subprojects: Vec<PathBuf>
}

pub struct Workspace {
    workspace_dir: PathBuf,
    projects: Vec<Project>
}

impl Workspace {
    pub fn new(dir: &Path) -> Workspace {
        Workspace {
            workspace_dir: PathBuf::from(dir),
            projects: Vec::new()
        }
    }

    pub fn init_lua(&self, lua: &mlua::Lua) -> mlua::Result<()> {
        let workspace_dir = self.workspace_dir.clone();
        let project_dir_func = lua.create_function(move |lua, dir: String| {
            process_project_file(lua, dir, workspace_dir.as_path())
        })?;

        lua.load(r#"
            local ws_dir, process_project_dir = ...

            cobble = {
                workspace = {
                    dir = ws_dir
                },
                projects = {},
            }

            PROJECT = nil
            WORKSPACE = cobble.workspace

            project_stack = {}

            function start_project (name, dir)
                if (not name or #name == 0) and PROJECT then
                    error("Empty name is only allowed for the root project")
                end

                local full_name = "/" .. (name or "")

                if PROJECT then
                    full_name = PROJECT.name .. full_name
                    dir = dir or PROJECT.dir
                end

                dir = dir or WORKSPACE.dir

                if cobble.projects[full_name] then
                    error("Project " .. full_name .. " already exists!")
                end

                local project = {
                    name = name,
                    dir = dir,
                    build_envs = {},
                    tasks = {},
                    child_projects = {}
                }
                
                if PROJECT then table.insert(PROJECT.child_projects, project) end

                cobble.projects[full_name] = project
                table.insert(cobble.project_stack, project)
                PROJECT = project
            end

            function end_project ()
                table.remove(cobble.project_stack)
                PROJECT = cobble.project_stack[#cobble.project_stack]
            end

            function project (name, def_func)
                start_project(name)
                def_func()
                end_project()
            end

            function project_dir (dir)
                process_project_dir(dir)
            end

            function build_env (env)
                table.insert(PROJECT.build_envs, env)
            end

            function task (tsk)
                table.insert(PROJECT.tasks, tsk)
            end
        "#).call((self.workspace_dir.as_os_str().to_str(), project_dir_func))
    }

    pub fn load_project_def_chunk<'lua>(&self, lua: &'lua mlua::Lua, project_path: &Path, project_def: mlua::Chunk<'lua, '_>) -> mlua::Result<Project> {
        lua.scope(|scope| {
            let build_envs: Rc<RefCell<Vec<BuildEnv>>> = Rc::new(RefCell::new(Vec::new()));
            let tasks: Rc<RefCell<Vec<Task>>> = Rc::new(RefCell::new(Vec::new()));
            let subprojects: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));

            let globals = lua.create_table()?;
            for pairs in lua.globals().pairs() {
                let (k, v): (mlua::Value, mlua::Value) = pairs?;
                globals.set(k, v)?;
            }

            let build_envs_clone = build_envs.clone();
            let build_env_func = scope.create_function(move |_lua, build_env: BuildEnv| {
                build_envs_clone.borrow_mut().push(build_env);
                Ok(())
            })?;

            let tasks_clone = tasks.clone();
            let task_func = scope.create_function(move |_lua, task: Task| {
                tasks_clone.borrow_mut().push(task);
                Ok(())
            })?;

            let subprojects_clone = subprojects.clone();
            let project_func = scope.create_function(move |_lua, subproject_path_str: String| {
                let subproject_path = project_path.join(subproject_path_str);
                subprojects_clone.borrow_mut().push(subproject_path);
                Ok(())
            })?;

            globals.set("build_env", build_env_func)?;
            globals.set("task", task_func)?;
            globals.set("subproject", project_func)?;

            let subproject_def = project_def.set_environment(globals);
            subproject_def.exec()?;

            Ok(Project {
                path: PathBuf::from(project_path),
                build_envs: build_envs.take(),
                tasks: tasks.take(),
                subprojects: subprojects.take()
            })
        })
    }
}

fn process_project_file(lua: &mlua::Lua, dir: String, workspace_dir: &Path) -> mlua::Result<()> {
    let current_project: Option<mlua::Table> = lua.globals().get("PROJECT")?;

    let project_dir = match current_project.as_ref() {
        Some(cur_proj) => {
            let current_project_dir: String = cur_proj.get("dir")?;
            Path::new(current_project_dir.as_str()).join(&dir)
        },
        None => PathBuf::from(dir.clone())
    };

    let project_name = if workspace_dir == Path::new(dir.as_str()) {
        String::new()
    } else {
        dir.clone()
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

    let start_project: mlua::Function = lua.globals().get("start_project")?;
    let end_project: mlua::Function = lua.globals().get("end_project")?;

    start_project.call((project_name, project_dir.as_os_str().to_str()))?;

    lua.load(project_source).exec()?;

    end_project.call(())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_init_lua() {
        let workspace = Workspace::new(Path::new("test"));
        let lua = create_lua_env().unwrap();

        workspace.init_lua(&lua).unwrap();

        let ws_dir: String = lua.load(r#"WORKSPACE.dir"#).eval().unwrap();
        assert_eq!(ws_dir, "test");
    }

    #[test]
    fn test_load_subproject_def() {
        let mut workspace = Workspace::new(Path::new("."));
        let lua = create_lua_env().unwrap();

        let project = workspace.load_project_def_chunk(&lua, Path::new("testproject"), lua.load(r#"
            build_env({
                name = "test",
                install_actions = {
                    function () print("hi!") end
                },
                install_deps = {},
                exec = function (a) print(a) end
            })
        "#)).unwrap();

        assert_eq!(project.build_envs.len(), 1);
        assert_eq!(project.tasks.len(), 0);
        assert_eq!(project.path, Path::new("testproject"))
    }
}