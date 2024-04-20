use std::fmt;
use std::collections::HashSet;

use crate::cobble::datamodel::{
    Action,
    ActionCmd,
    Dependency,
    DependencyList,
};
use crate::cobble::lua::detached_value::dump_function;

#[derive(Debug)]
pub struct BuildEnv {
    pub name: String,
    pub install_actions: Vec<ActionCmd>,
    pub install_deps: Vec<Dependency>,
    pub action: Action,
}

impl fmt::Display for BuildEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuildEnv(")?;
        write!(f, "name={}, ", &self.name)?;
        write!(f, "install_actions=[")?;
        for (i, action) in self.install_actions.iter().enumerate() {
            if i > 0 { f.write_str(",")?; }
            write!(f, "{}", action)?;
        }
        f.write_str("])")
    }
}

impl <'lua> mlua::FromLua<'lua> for BuildEnv {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let build_env_def_table = match value {
            mlua::Value::Table(tbl) => tbl,
            val => { return Err(mlua::Error::RuntimeError(format!("Unable to convert value to a BuildEnvDef: {:?}", val))); }
        };

        let name: String = build_env_def_table.get("name")?;

        let install_actions: Vec<ActionCmd> = build_env_def_table.get("install_actions")?;
        let install_deps: Vec<Dependency> = build_env_def_table.get::<_, DependencyList>("install_deps")?.0;
        let action: Action = build_env_def_table.get("action")?;

        Ok(BuildEnv {
            name,
            install_actions,
            install_deps,
            action
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_build_env_def_from_lua_table() {
        let lua = create_lua_env(Path::new(".")).unwrap();

        let build_env_table: mlua::Table = lua.load(r#"
            {
                name = "poetry",
                install_actions = {
                    {"poetry", "lock"},
                    {"poetry", "install"}
                },
                install_deps = {
                    files = {"pyproject.toml", "poetry.lock"}
                },
                exec = function (args) cmd("poetry", table.unpack(args)) end
            }
        "#).eval().unwrap();

        let build_env: BuildEnv = lua.unpack(mlua::Value::Table(build_env_table)).unwrap();
        assert_eq!(build_env.name, String::from("poetry"));
    }
}