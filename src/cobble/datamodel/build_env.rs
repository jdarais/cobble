use std::fmt;

use crate::cobble::datamodel::{
    Action,
    Dependency,
    DependencyList,
};

#[derive(Clone, Debug)]
pub struct BuildEnv {
    pub name: String,
    pub install: Vec<Action>,
    pub deps: Vec<Dependency>,
    pub action: Action,
}

impl fmt::Display for BuildEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuildEnv(")?;
        write!(f, "name={}, ", &self.name)?;

        f.write_str("install=[")?;
        for (i, action) in self.install.iter().enumerate() {
            if i > 0 { f.write_str(",")?; }
            write!(f, "{}", action)?;
        }
        f.write_str("], ")?;


        f.write_str("deps=[")?;
        for (i, dep) in self.deps.iter().enumerate() {
            if i > 0 { f.write_str(",")?; }
            write!(f, "{}", dep)?;
        }
        f.write_str("], ")?;

        write!(f, "action={})", self.action)
    }
}

impl <'lua> mlua::FromLua<'lua> for BuildEnv {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let name: String = tbl.get("name")?;

                let install: Vec<Action> = tbl.get("install")?;
                let deps_opt: Option<DependencyList> = tbl.get("deps")?;
                let deps = deps_opt.map(|d| d.0).unwrap_or_default();
                let action: Action = tbl.get("action")?;
        
                Ok(BuildEnv { name, install, deps, action })
            },
            mlua::Value::UserData(val) => Ok(val.borrow::<BuildEnv>()?.clone()),
            val => { return Err(mlua::Error::runtime(format!("Unable to convert value to a BuildEnvDef: {:?}", val))); }
        }
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
                install = {
                    {"poetry", "lock"},
                    {"poetry", "install"}
                },
                deps = {
                    files = {"pyproject.toml", "poetry.lock"}
                },
                action = function (args) cmd("poetry", table.unpack(args)) end
            }
        "#).eval().unwrap();

        let build_env: BuildEnv = lua.unpack(mlua::Value::Table(build_env_table)).unwrap();
        assert_eq!(build_env.name, String::from("poetry"));
    }
}