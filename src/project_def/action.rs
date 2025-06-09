// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use crate::lua::s11n::{to_ser_lua_value, SerLuaValueBlock, SerLuaValueRef, SerLuaValueType};
use crate::project_def::validate::{
    prop_path_string, push_prop_name_if_exists, validate_is_string, validate_is_table,
    validate_table_has_only_string_or_sequence_keys, validate_table_is_sequence,
    validate_table_value_type, with_prop, ValidationError,
};

#[derive(Clone, Debug)]
pub enum ActionCmd {
    Cmd(Vec<Arc<str>>),
    Func(SerLuaValueBlock),
}

impl fmt::Display for ActionCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ActionCmd::*;
        match self {
            Cmd(args) => write!(f, "Cmd({})", args.join(",")),
            Func(func) => write!(f, "Func({:?})", func),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Action {
    pub tools: HashMap<Arc<str>, Arc<str>>,
    pub build_envs: HashMap<Arc<str>, Arc<str>>,
    pub kwargs: HashMap<Arc<str>, SerLuaValueBlock>,
    pub cmd: ActionCmd,
}

fn validate_name_alias_table<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    match value {
        SerLuaValueRef::Table(tbl_val) => {
            validate_table_has_only_string_or_sequence_keys(tbl_val, &mut *prop_path)?;
            validate_table_value_type(tbl_val, &vec![SerLuaValueType::String], &mut *prop_path)
        }
        SerLuaValueRef::String(_) => Ok(()),
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().cloned().map(|p| p.into()).collect(),
            expected: vec![SerLuaValueType::Table, SerLuaValueType::String],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
    }
}

pub fn validate_action_list<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    let tbl_val = validate_is_table(value, &mut *prop_path)?;
    validate_table_is_sequence(tbl_val, &mut *prop_path)?;
    for (k, v) in tbl_val.entries() {
        with_prop(&mut *prop_path, k, |path| {
            validate_action(&v, path)
        })?;
    }
    Ok(())
}

pub fn validate_action<'a>(
    value: &SerLuaValueRef<'a>,
    prop_path: &mut Vec<SerLuaValueRef<'a>>,
) -> Result<(), ValidationError> {
    match value {
        SerLuaValueRef::Table(tbl_val) => {
            let mut sequence_items: BTreeMap<SerLuaValueRef<'a>, SerLuaValueRef<'a>> =
                BTreeMap::new();
            for (k, v) in tbl_val.entries() {
                match k {
                    SerLuaValueRef::Integer(i) => {
                        sequence_items.insert(k, v);
                        Ok(())
                    }
                    SerLuaValueRef::String(ks) => match ks {
                        "tool" => {
                            with_prop(&mut *prop_path, SerLuaValueRef::String("tool"), |path| {
                                validate_name_alias_table(&v, path)
                            })
                        }
                        "env" => {
                            with_prop(&mut *prop_path, SerLuaValueRef::String("env"), |path| {
                                validate_name_alias_table(&v, path)
                            })
                        }
                        _ => Ok(()),
                    },
                    _ => Err(ValidationError::InvalidType {
                        path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                        expected: vec![SerLuaValueType::Integer, SerLuaValueType::String],
                        actual: value.value_type(),
                        value: value.clone().into()
                    }),
                }?;
            }

            let first_seq_val_opt = sequence_items.remove(&SerLuaValueRef::Integer(1));
            match first_seq_val_opt {
                Some(first_seq_val) => match first_seq_val {
                    SerLuaValueRef::Function(_) => match sequence_items.len() { 
                        0 => Ok(()),
                        _ => Err(ValidationError::InvalidValue {
                            path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                            value: value.clone().into(),
                            message: String::from("For function actions, the function is the only allowed positional element")
                        })
                    }
                    SerLuaValueRef::String(_) => {
                        for (k, v) in sequence_items.iter() {
                            with_prop(&mut *prop_path, k.clone(), |path| validate_is_string(v, path))?;
                        }
                        Ok(())
                    }
                    _ => Err(ValidationError::InvalidValue{
                        path: prop_path.iter().cloned().map(|p| p.into()).collect(),
                        value: value.clone().into(),
                        message: format!("Expected a string or function as the first sequence item, but got a {}: (value={:?})", first_seq_val.value_type(), first_seq_val)
                    })
                }
                None => Ok(()),
            }?;


            Ok(())
        }
        SerLuaValueRef::Function(_) => Ok(()),
        _ => Err(ValidationError::InvalidType {
            path: prop_path.iter().cloned().map(|p| p.into()).collect(),
            expected: vec![SerLuaValueType::Function, SerLuaValueType::Table],
            actual: value.value_type(),
            value: value.clone().into(),
        }),
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Action(")?;

        if self.build_envs.len() > 0 {
            f.write_str("envs={")?;
            for (i, (env_alias, env_name)) in self.build_envs.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}: {}", env_alias, env_name)?;
            }
            f.write_str("}, ")?;
        }

        if self.tools.len() > 0 {
            f.write_str("tools={")?;
            for (i, (tool_alias, tool_name)) in self.tools.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                write!(f, "{}: {}", tool_alias, tool_name)?;
            }
            f.write_str("}, ")?;
        }

        write!(f, "cmd={})", self.cmd)
    }
}

impl<'lua> mlua::FromLua<'lua> for Action {
    fn from_lua(
        value: mlua::prelude::LuaValue<'lua>,
        lua: &'lua mlua::prelude::Lua,
    ) -> mlua::prelude::LuaResult<Self> {
        match value {
            mlua::Value::Table(tbl) => {
                let mut build_envs: HashMap<Arc<str>, Arc<str>> = HashMap::new();
                let mut tools: HashMap<Arc<str>, Arc<str>> = HashMap::new();
                let mut kwargs: HashMap<Arc<str>, SerLuaValueBlock> = HashMap::new();

                for pair in tbl.clone().pairs() {
                    let (k, v): (mlua::Value, mlua::Value) = pair?;
                    if let mlua::Value::String(s) = k {
                        match s.to_str()? {
                            "env" => match v {
                                mlua::Value::String(s) => {
                                    let build_env_name = Arc::<str>::from(s.to_str()?);
                                    build_envs.insert(build_env_name.clone(), build_env_name);
                                }
                                mlua::Value::Table(build_env_tbl) => {
                                    for pair in build_env_tbl.pairs() {
                                        let (k_val, v_str): (mlua::Value, String) = pair?;
                                        let v = Arc::<str>::from(v_str);
                                        let k = match k_val {
                                            mlua::Value::String(s) => Arc::<str>::from(s.to_str()?),
                                            _ => v.clone(),
                                        };
                                        build_envs.insert(k, v);
                                    }
                                }
                                mlua::Value::Nil => { /* no build envs to add */ }
                                _ => {
                                    return Err(mlua::Error::runtime(format!(
                                        "Invalid value for 'env' property: {:?}",
                                        v
                                    )));
                                }
                            },
                            "tool" => match v {
                                mlua::Value::String(s) => {
                                    let tool_name = Arc::<str>::from(s.to_str()?);
                                    tools.insert(tool_name.clone(), tool_name);
                                }
                                mlua::Value::Table(tool_tbl) => {
                                    for pair in tool_tbl.pairs() {
                                        let (k_val, v_str): (mlua::Value, String) = pair?;
                                        let v = Arc::<str>::from(v_str);
                                        let k = match k_val {
                                            mlua::Value::String(s) => Arc::<str>::from(s.to_str()?),
                                            _ => v.clone(),
                                        };
                                        tools.insert(k, v);
                                    }
                                }
                                mlua::Value::Nil => { /* no tools to add */ }
                                _ => {
                                    return Err(mlua::Error::runtime(format!(
                                        "Invalid value for 'tool' property: {:?}",
                                        v
                                    )))
                                }
                            },
                            kwarg => {
                                let detached_v = to_ser_lua_value(lua, &v)?;
                                kwargs.insert(Arc::<str>::from(kwarg.to_owned()), detached_v);
                            }
                        }
                    }
                }

                let cmd_tool_name = Arc::<str>::from("cmd");

                // Check if we are a table with a single positional element, which could mean that
                // we're a function action
                if tbl.len()? == 1 {
                    let first_val: mlua::Value = tbl.get(1)?;
                    match first_val {
                        mlua::Value::Function(func) => {
                            // Function actions should always have the cmd tool available
                            if !tools.contains_key(&cmd_tool_name) {
                                tools.insert(cmd_tool_name.clone(), cmd_tool_name);
                            }

                            let ser_lua_fn = to_ser_lua_value(lua, &mlua::Value::Function(func))?;

                            return Ok(Action {
                                build_envs,
                                tools,
                                kwargs,
                                cmd: ActionCmd::Func(ser_lua_fn),
                            });
                        }
                        _ => { /* not a function action */ }
                    }
                }

                // Otherwise, interpret the contents of the table as an args list, in which case only one
                // build env or tool is allowed.  If no build env or tool is specified, use the "cmd" tool
                match build_envs.len() + tools.len() {
                    0 => {
                        tools.insert(cmd_tool_name.clone(), cmd_tool_name);
                    }
                    1 => { /* no action needed */ }
                    _ => {
                        return Err(mlua::Error::runtime(
                            "Can only use one env or tool with argument list action",
                        ));
                    }
                };

                let args_res: mlua::Result<Vec<String>> = tbl.clone().sequence_values().collect();
                let args = args_res?;

                Ok(Action {
                    build_envs,
                    tools,
                    kwargs,
                    cmd: ActionCmd::Cmd(args.into_iter().map(|s| Arc::<str>::from(s)).collect()),
                })
            }
            mlua::Value::Function(func) => {
                // We are a function action without a tool or build env
                // Function actions should always have the cmd tool available
                let cmd_tool_name = Arc::<str>::from("cmd");
                let ser_lua_fn = to_ser_lua_value(lua, &mlua::Value::Function(func))?;
                Ok(Action {
                    build_envs: HashMap::new(),
                    tools: vec![(cmd_tool_name.clone(), cmd_tool_name)]
                        .into_iter()
                        .collect(),
                    kwargs: HashMap::new(),
                    cmd: ActionCmd::Func(ser_lua_fn),
                })
            }
            _ => Err(mlua::Error::runtime(
                "Expected a lua table to convert to Action",
            )),
        }
    }
}

impl<'lua> mlua::IntoLua<'lua> for Action {
    fn into_lua(self, lua: &'lua mlua::prelude::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let Action {
            build_envs,
            tools,
            kwargs,
            cmd,
        } = self;

        let action_table = lua.create_table()?;

        match cmd {
            ActionCmd::Cmd(args) => {
                if build_envs.len() + tools.len() > 1 {
                    return Err(mlua::Error::runtime(
                        "Can only use one build_env or tool with an argument list action",
                    ));
                }

                for arg in args {
                    action_table.push(arg.as_ref())?;
                }
            }
            ActionCmd::Func(f) => {
                action_table.push(&f)?;
            }
        }

        for (k, v) in kwargs {
            action_table.set(k.as_ref(), v)?;
        }

        let tools_str: HashMap<&str, &str> = tools
            .iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect();
        let build_envs_str: HashMap<&str, &str> = build_envs
            .iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect();

        action_table.set("tool", tools_str)?;
        action_table.set("env", build_envs_str)?;

        Ok(mlua::Value::Table(action_table))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use crate::lua::lua_env::create_lua_env;

    #[test]
    fn test_validate_and_convert_arg_list_action() {
        let lua = create_lua_env(Path::new(".")).unwrap();
        let action_val: mlua::Value = lua
            .load(r#"{ tool = "cmd", "echo", "hi", "there" }"#)
            .eval()
            .unwrap();
        validate_action(&lua, &action_val, None, &mut Vec::new()).unwrap();
        let action: Action = lua.unpack(action_val).unwrap();
        match action.cmd {
            ActionCmd::Cmd(_) => { /* OK */ }
            cmd => {
                panic!("Expected an ActionCmd::Cmd, but got {:?}", cmd);
            }
        };
    }

    #[test]
    fn test_validate_and_convert_function_action() {
        let lua = create_lua_env(Path::new(".")).unwrap();
        let action_val: mlua::Value = lua
            .load(
                r#"
            {
                tool = "cmd",
                function (c) print("hello!") end
            }
        "#,
            )
            .eval()
            .unwrap();
        validate_action(&lua, &action_val, None, &mut Vec::new()).unwrap();
        let action: Action = lua.unpack(action_val).unwrap();
        match action.cmd {
            ActionCmd::Func(_) => { /* OK */ }
            cmd => {
                panic!("Expected an ActionCmd::Func, but got {:?}", cmd);
            }
        };
    }

    #[test]
    fn test_mixed_function_and_string_sequence_fails_validation() {
        let lua = create_lua_env(Path::new(".")).unwrap();
        let action_val: mlua::Value = lua
            .load(
                r#"
                {
                    tool = "cmd",
                    function (c) print("hello!") end,
                    "wait",
                    "what",
                    "but why"
                }
                "#,
            )
            .eval()
            .unwrap();
        validate_action(&lua, &action_val, None, &mut Vec::new()).expect_err(
            "Validation of action with mixed function and string sequence values should fail",
        );

        lua.unpack::<Action>(action_val)
            .expect_err("Conversion of action that failed validation should also fail conversion to an Action object");
    }
}
