extern crate mlua;

use std::cell::RefCell;
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
    projects: Vec<Project>
}

impl Workspace {
    pub fn new() -> Workspace {
        Workspace {
            projects: Vec::new()
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_load_subproject_def() {
        let mut workspace = Workspace::new();
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