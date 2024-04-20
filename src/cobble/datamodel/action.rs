use std::collections::HashSet;
use std::fmt;

use crate::cobble::datamodel::BuildEnv;
use crate::cobble::lua::detached_value::{FunctionDump, dump_function};

#[derive(Clone, Debug)]
pub enum ActionCmd {
    Cmd(Vec<String>),
    Func(FunctionDump)
}

impl fmt::Display for ActionCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ActionCmd::*;
        match self {
            Cmd(args) => write!(f, "Cmd({})", args.join(",")),
            Func(func) => write!(f, "Func({})", func)
        }
    }
}

impl <'lua> mlua::FromLua<'lua> for ActionCmd {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Function(func) => Ok(ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)),
            mlua::Value::Table(tbl) => {
                let cmd_args_res: mlua::Result<Vec<String>> = tbl.sequence_values().collect();
                let cmd_args = cmd_args_res?;
                Ok(ActionCmd::Cmd(cmd_args))
            },
            val => Err(mlua::Error::RuntimeError(format!("Unable to convert value to Action: {:?}", val)))
        }
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    build_env: Option<String>,
    cmd: ActionCmd
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action(")?;

        if let Some(build_env) = &self.build_env {
            write!(f, "build_env={},", build_env)?;
        }

        write!(f, "cmd={})", self.cmd)
    }
}

impl <'lua> mlua::FromLua<'lua> for Action {
    fn from_lua(value: mlua::prelude::LuaValue<'lua>, lua: &'lua mlua::prelude::Lua) -> mlua::prelude::LuaResult<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let build_env: Option<String> = tbl.get("build_env")?;

                // Check if we are a table with a single positional element, which means that we're
                // a function command, (likely accompanied by a build_env entry in the table)
                if tbl.len()? == 1 {
                    if let mlua::Value::Function(func) = tbl.get(1)? {
                        return Ok(Action {
                            build_env,
                            cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                        });
                    }
                }

                let args_res: mlua::Result<Vec<String>> = tbl.clone().sequence_values().collect();
                let args = args_res?;

                Ok(Action {
                    build_env,
                    cmd: ActionCmd::Cmd(args)
                })
            },
            mlua::Value::Function(func) => {
                // We are a function action without a build env
                Ok(Action {
                    build_env: None,
                    cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                })
            },
            _ => Err(mlua::Error::RuntimeError(String::from("Expected a lua table to convert to Action")))
        }
    }
}
