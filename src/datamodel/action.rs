use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::lua::detached_value::{FunctionDump, dump_function};

#[derive(Clone, Debug)]
pub enum ActionCmd {
    Cmd(Vec<Arc<str>>),
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
                let cmd_args: Vec<String> = cmd_args_res?;
                let cmd_args_arc_str: Vec<Arc<str>> = cmd_args.into_iter().map(|s| Arc::<str>::from(s)).collect();
                Ok(ActionCmd::Cmd(cmd_args_arc_str))
            },
            val => Err(mlua::Error::runtime(format!("Unable to convert value to Action: {:?}", val)))
        }
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    pub tools: HashMap<Arc<str>, Arc<str>>,
    pub build_envs: HashMap<Arc<str>, Arc<str>>,
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
                let mut build_envs: HashMap<Arc<str>, Arc<str>> = HashMap::new();
                match build_env_val {
                    mlua::Value::String(s) => {
                        let build_env_name = Arc::<str>::from(s.to_str()?);
                        build_envs.insert(build_env_name.clone(), build_env_name);
                    },
                    mlua::Value::Table(build_env_tbl) => {
                        for pair in build_env_tbl.pairs() {
                            let (k_val, v_str): (mlua::Value, String) = pair?;
                            let v = Arc::<str>::from(v_str);
                            let k = match k_val {
                                mlua::Value::String(s) => Arc::<str>::from(s.to_str()?),
                                _ => v.clone()
                            };
                            build_envs.insert(k, v);
                        }
                    },
                    mlua::Value::Nil => { /* no build envs to add */},
                    _ => { return Err(mlua::Error::runtime(format!("Invalid value for 'build_env' property: {:?}", build_env_val))); }
                }

                let tool_val: mlua::Value = tbl.get("tool")?;
                let mut tools: HashMap<Arc<str>, Arc<str>> = HashMap::new();
                match tool_val {
                    mlua::Value::String(s) => {
                        let tool_name = Arc::<str>::from(s.to_str()?);
                        tools.insert(tool_name.clone(), tool_name);
                    },
                    mlua::Value::Table(tool_tbl) => {
                        for pair in tool_tbl.pairs() {
                            let (k_val, v_str): (mlua::Value, String) = pair?;
                            let v = Arc::<str>::from(v_str);
                            let k = match k_val {
                                mlua::Value::String(s) => Arc::<str>::from(s.to_str()?),
                                _ => v.clone()
                            };
                            tools.insert(k, v);
                        }
                    },
                    mlua::Value::Nil => { /* no tools to add */},
                    _ => { return Err(mlua::Error::runtime(format!("Invalid value for 'tool' property: {:?}", tool_val)))}
                }
                
                let cmd_tool_name = Arc::<str>::from("cmd");

                // Check if we are a table with a single positional element, which means that we're
                // a function command, (likely accompanied by a build_env or tool entry in the table)
                if tbl.len()? == 1 {
                    let first_val: mlua::Value = tbl.get(1)?;
                    if let mlua::Value::Function(func) = first_val {
                        // Function actions should always have the cmd tool available
                        if !tools.contains_key(&cmd_tool_name) {
                            tools.insert(cmd_tool_name.clone(), cmd_tool_name);
                        }

                        return Ok(Action {
                            build_envs,
                            tools,
                            cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                        });
                    }
                }

                // Otherwise, interpret the contents of the table as an args list, in which case only one
                // build env or tool is allowed.  If no build env or tool is specified, use the "cmd" tool
                match build_envs.len() + tools.len() {
                    0 => {
                        tools.insert(cmd_tool_name.clone(), cmd_tool_name);
                    },
                    1 => { /* no action needed */},
                    _ => { return Err(mlua::Error::runtime("Can only use one build_env or tool with argument list action")); }
                };

                let args_res: mlua::Result<Vec<String>> = tbl.clone().sequence_values().collect();
                let args = args_res?;

                Ok(Action {
                    build_envs,
                    tools,
                    cmd: ActionCmd::Cmd(args.into_iter().map(|s| Arc::<str>::from(s)).collect())
                })
            },
            mlua::Value::Function(func) => {
                // We are a function action without a tool or build env
                // Function actions should always have the cmd tool available
                let cmd_tool_name = Arc::<str>::from("cmd");
                Ok(Action {
                    build_envs: HashMap::new(),
                    tools: vec![(cmd_tool_name.clone(), cmd_tool_name)].into_iter().collect(),
                    cmd: ActionCmd::Func(dump_function(func, lua, &HashSet::new())?)
                })
            },
            _ => Err(mlua::Error::runtime("Expected a lua table to convert to Action"))
        }
    }
}

impl <'lua> mlua::IntoLua<'lua> for Action {
    fn into_lua(self, lua: &'lua mlua::prelude::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let Action {build_envs, tools, cmd} = self;

        let action_table = lua.create_table()?;

        match cmd {
            ActionCmd::Cmd(args) => {
                if build_envs.len() + tools.len() > 1 {
                    return Err(mlua::Error::runtime("Can only use one build_env or tool with an argument list action"));
                }

                for arg in args {
                    action_table.push(arg.as_ref())?;
                }
            },
            ActionCmd::Func(f) => {
                action_table.push(f)?;
            }
        }

        let tools_str: HashMap<&str, &str> = tools.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();
        let build_envs_str: HashMap<&str, &str> = build_envs.iter().map(|(k, v)| (k.as_ref(), v.as_ref())).collect();

        action_table.set("tool", tools_str)?;
        action_table.set("build_env", build_envs_str)?;

        Ok(mlua::Value::Table(action_table))
    }
}
