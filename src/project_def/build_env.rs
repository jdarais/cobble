use std::borrow::Cow;
use std::{fmt, sync::Arc};

use crate::project_def::action::{validate_action, validate_action_list};
use crate::project_def::dependency::{validate_dep_list, Dependencies};
use crate::project_def::validate::{key_validation_error, validate_is_string, validate_required_key};
use crate::project_def::Action;

#[derive(Clone, Debug)]
pub struct BuildEnv {
    pub name: Arc<str>,
    pub install: Vec<Action>,
    pub clean: Vec<Action>,
    pub deps: Dependencies,
    pub action: Action,
}


pub fn validate_build_env<'lua>(lua: &'lua mlua::Lua, value: &mlua::Value) -> mlua::Result<()> {
    let mut prop_path: Vec<Cow<str>> = Vec::new();
    match value {
        mlua::Value::Table(tbl_val) => {
            validate_required_key(tbl_val, "name", None, &mut prop_path)?;
            validate_required_key(tbl_val, "install", None, &mut prop_path)?;
            validate_required_key(tbl_val, "action", None, &mut prop_path)?;

            for pair in tbl_val.clone().pairs() {
                let (k, v): (mlua::Value, mlua::Value) = pair?;
                let k_str = validate_is_string(&k, None, &mut prop_path)?;
                match k_str.to_str()? {
                    "name" => validate_is_string(&v, Some(Cow::Borrowed("name")), &mut prop_path).and(Ok(())),
                    "install" => validate_action_list(lua, &v, Some(Cow::Borrowed("install")), &mut prop_path),
                    "clean" => validate_action_list(lua, &v, Some(Cow::Borrowed("clean")), &mut prop_path),
                    "deps" => validate_dep_list(lua, &v, Some(Cow::Borrowed("deps")), &mut prop_path),
                    "action" => validate_action(lua, &v, Some(Cow::Borrowed("action")), &mut prop_path),
                    s_str => key_validation_error(s_str, vec!["name", "install", "deps"], &mut prop_path)
                }?;
            }

            Ok(())
        },
        _ => Err(mlua::Error::runtime(format!("Expected a table, but got a {}: {:?}", value.type_name(), value)))
    }
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


        write!(f, "deps={}", self.deps)?;

        write!(f, "action={})", self.action)
    }
}

impl <'lua> mlua::FromLua<'lua> for BuildEnv {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let name_str: String = tbl.get("name")?;
                let name = Arc::<str>::from(name_str);

                let install: Vec<Action> = tbl.get("install")?;
                let clean_opt: Option<Vec<Action>> = tbl.get("clean")?;
                let clean = clean_opt.unwrap_or_default();
                let deps_opt: Option<Dependencies> = tbl.get("deps")?;
                let deps: Dependencies = deps_opt.unwrap_or_default();
                let action: Action = tbl.get("action")?;
        
                Ok(BuildEnv { name, install, clean, deps, action })
            },
            val => { return Err(mlua::Error::runtime(format!("Unable to convert value to a BuildEnvDef: {:?}", val))); }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use crate::lua::lua_env::create_lua_env;

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
        assert_eq!(build_env.name, Arc::<str>::from("poetry"));
    }
}