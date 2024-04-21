use std::collections::{HashMap, HashSet};
use std::fmt;

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
            val => Err(mlua::Error::runtime(format!("Unable to convert value to Action: {:?}", val)))
        }
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    pub tools: HashMap<String, String>,
    pub build_envs: HashMap<String, String>,
    pub cmd: ActionCmd
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action(")?;

        if self.build_envs.len() > 0 {
            f.write_str("build_envs={")?;
            for (i, (env_alias, env_name)) in self.build_envs.iter().enumerate() {
                if i > 0 { f.write_str(", ")?; }
                write!(f, "{}: {}", env_alias, env_name)?;
            }
            f.write_str("}, ")?;
        }

        if self.tools.len() > 0 {
            f.write_str("tools={")?;
            for (i, (tool_alias, tool_name)) in self.tools.iter().enumerate() {
                if i > 0 { f.write_str(", ")?; }
                write!(f, "{}: {}", tool_alias, tool_name)?;
            }
            f.write_str("}, ")?;
        }

        write!(f, "cmd={})", self.cmd)
    }
}

impl <'lua> mlua::FromLua<'lua> for Action {
    fn from_lua(value: mlua::prelude::LuaValue<'lua>, lua: &'lua mlua::prelude::Lua) -> mlua::prelude::LuaResult<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let build_env_val: mlua::Value = tbl.get("build_env")?;
                let mut build_envs: HashMap<String, String> = HashMap::new();
                match build_env_val {
                    mlua::Value::String(s) => {
                        build_envs.insert(String::from(s.to_str()?), String::from(s.to_str()?));
                    },
                    mlua::Value::Table(build_env_tbl) => {
                        for pair in build_env_tbl.pairs() {
                            let (k_val, v): (mlua::Value, String) = pair?;
                            let k = match k_val {
                                mlua::Value::String(s) => String::from(s.to_str()?),
                                _ => v.clone()
                            };
                            build_envs.insert(k, v);
                        }
                    },
                    mlua::Value::Nil => { /* no build envs to add */},
                    _ => { return Err(mlua::Error::runtime(format!("Invalid value for 'build_env' property: {:?}", build_env_val))); }
                }

                let tool_val: mlua::Value = tbl.get("tool")?;
                let mut tools: HashMap<String, String> = HashMap::new();
                match tool_val {
                    mlua::Value::String(s) => {
                        tools.insert(String::from(s.to_str()?), String::from(s.to_str()?));
                    },
                    mlua::Value::Table(tool_tbl) => {
                        for pair in tool_tbl.pairs() {
                            let (k_val, v): (mlua::Value, String) = pair?;
                            let k = match k_val {
                                mlua::Value::String(s) => String::from(s.to_str()?),
                                _ => v.clone()
                            };
                            tools.insert(k, v);
                        }
                    },
                    mlua::Value::Nil => { /* no tools to add */},
                    _ => { return Err(mlua::Error::runtime(format!("Invalid value for 'tool' property: {:?}", tool_val)))}
                }
                

                // Check if we are a table with a single positional element, which means that we're
                // a function command, (likely accompanied by a build_env or tool entry in the table)
                if tbl.len()? == 1 {
                    let first_val: mlua::Value = tbl.get(1)?;
                    if let mlua::Value::Function(func) = first_val {
                        return Ok(Action {
                            build_envs,
                            tools,
                            cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                        });
                    }
                }

                // Otherwise, interpret the contents of the table as an args list, in which case only one
                // build env or tool is allowed
                if build_envs.len() + tools.len() > 1 {
                    return Err(mlua::Error::runtime("Can only use one build_env or tool with argument list action"));
                }

                let args_res: mlua::Result<Vec<String>> = tbl.clone().sequence_values().collect();
                let args = args_res?;

                Ok(Action {
                    build_envs,
                    tools,
                    cmd: ActionCmd::Cmd(args)
                })
            },
            mlua::Value::Function(func) => {
                // We are a function action without a tool or build env
                Ok(Action {
                    build_envs: HashMap::new(),
                    tools: HashMap::new(),
                    cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                })
            },
            _ => Err(mlua::Error::runtime("Expected a lua table to convert to Action"))
        }
    }
}
