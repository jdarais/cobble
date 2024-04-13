extern crate mlua;

use std::sync::{Arc, Mutex};

use crate::cobble::datamodel::BuildEnv;

pub struct Workspace {
    build_envs: Arc<Mutex<Vec<BuildEnv>>>,
}

impl Workspace {
    pub fn new() -> Workspace {
        Workspace {
            build_envs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_build_env_def(&self, build_env_def: BuildEnv) {
        let mut build_envs = self.build_envs.lock().unwrap();
        build_envs.push(build_env_def);
    }

    pub fn load_subproject_def<'lua>(&self, lua: &'lua mlua::Lua, subproject_def: mlua::Chunk<'lua, '_>) -> mlua::Result<()> {
        
        lua.scope(|scope| {
            let build_envs_mutex = self.build_envs.clone();
            let build_env_func = scope.create_function(move |_lua, build_env: BuildEnv| {
                let mut build_envs = build_envs_mutex.lock().unwrap();
                build_envs.push(build_env);
                Ok(())
            })?;
            
            let globals = lua.globals().clone();
            globals.set("build_env", build_env_func)?;

            let subproject_def = subproject_def.set_environment(globals);
            subproject_def.exec()?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_load_subproject_def() {
        let workspace = Workspace::new();
        let lua = create_lua_env().unwrap();

        workspace.load_subproject_def(&lua, lua.load(r#"
            build_env({
                name = "test",
                install_actions = {
                    function () print("hi!") end
                },
                install_deps = {},
                exec = function (a) print(a) end
            })
        "#)).unwrap();

        let build_envs = workspace.build_envs.lock().unwrap();
        assert_eq!(build_envs.len(), 1);
    }
}