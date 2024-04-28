use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::os::raw::c_void;

#[derive(Clone, Debug)]
pub enum DetachedLuaValue {
    Nil,
    Boolean(bool),
    LightUserData(mlua::LightUserData),
    Integer(mlua::Integer),
    Number(mlua::Number),
    String(String),
    Table(HashMap<DetachedLuaValue, DetachedLuaValue>),
    Function(FunctionDump),
    UserData(*const c_void),
    Error(mlua::Error)
}

impl fmt::Display for DetachedLuaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use DetachedLuaValue::*;
        match self {
            Nil => f.write_str("nil"),
            Boolean(val) => f.write_str(if *val { "true" } else { "false" }),
            LightUserData(val) => write!(f, "LUD({:?})", val.0),
            Integer(val) => write!(f, "{}", val),
            Number(val) => write!(f, "{}", val),
            String(val) => write!(f, "\"{}\"", val.as_str()),
            Table(val) => {
                f.write_str("{")?;
                for (i, (k, v)) in val.iter().enumerate() {
                    if i > 0 { f.write_str(", ")?; }
                    write!(f, "{}: {}", k, v)?;
                }
                f.write_str("}")
            }
            Function(val) => write!(f, "{}", val),
            UserData(val) => write!(f, "UD({:?})", val),
            Error(val) => write!(f, "Error({})", val)
        }
    }
}

impl Eq for DetachedLuaValue {}
impl PartialEq for DetachedLuaValue {
    fn eq(&self, other: &Self) -> bool {
        use DetachedLuaValue::*;
        match self {
            Nil => match other {
                Nil => true,
                _ => false
            },
            Boolean(self_val) => match other {
                Boolean(other_val) => self_val == other_val,
                _ => false
            },
            LightUserData(this_val) => match other {
                LightUserData(other_val) => this_val.0 == other_val.0,
                _ => false
            },
            Integer(this_val) => match other {
                Integer(other_val) => this_val == other_val,
                _ => false
            },
            Number(this_val) => match other {
                Number(other_val) => this_val.to_bits() == other_val.to_bits(),
                _ => false
            },
            String(this_val) => match other {
                String(other_val) => this_val == other_val,
                _ => false
            },
            Table(this_val) => match other {
                Table(other_val) => this_val == other_val,
                _ => false
            },
            Function(this_val) => match other {
                Function(other_val) => this_val == other_val,
                _ => false
            },
            UserData(this_val) => match other {
                UserData(other_val) => this_val == other_val,
                _ => false
            },
            Error(this_val) => match other {
                Error(other_val) => this_val.to_string() == other_val.to_string(),
                _ => false
            }
        }
    }
}

impl Hash for DetachedLuaValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use DetachedLuaValue::*;
        match self {
            Nil => { state.write(b"nil"); },
            Boolean(val) => {
                state.write(b"bln");
                val.hash(state);
            },
            LightUserData(val) => {
                state.write(b"lud");
                val.0.hash(state);
            },
            Integer(val) => {
                state.write(b"int");
                val.hash(state);
            },
            Number(val) => {
                state.write(b"num");
                val.to_bits().hash(state);
            },
            String(val) => {
                state.write(b"str");
                val.hash(state);
            },
            Table(tbl) => {
                state.write(b"tbl");
                for (k, v) in tbl.iter() {
                    state.write(b"key");
                    k.hash(state);
                    state.write(b"val");
                    v.hash(state);
                }
            },
            Function(f) => {
                state.write(&f.source[..]);
                for upval in f.upvalues.iter() {
                    upval.hash(state);
                }
            },
            UserData(val) => {
                state.write(b"usd");
                val.hash(state);
            },
            Error(val) => {
                state.write(b"err");
                val.to_string().hash(state);
            }
        }

    }
}

pub fn dump_function<'lua>(func: mlua::Function<'lua>, lua: &'lua mlua::Lua, history: &HashSet<*const c_void>) -> Result<FunctionDump, mlua::Error> {
    if history.contains(&func.to_pointer()) {
        return Err(mlua::Error::runtime(format!("Cycle encountered when extracting Function: {:?}", &func)));
    }

    if func.info().what != "Lua" {
        return Err(mlua::Error::runtime(format!("Cannot serialize a function that is not a pure Lua function: {:?}", &func)));
    }

    let string_dump: mlua::Function = lua.load("function(fn) return string.dump(fn) end").eval()?;
    let source_str: mlua::String = string_dump.call(func.clone())?;
    let source = source_str.as_bytes().to_owned();

    let get_upvalues: mlua::Function = lua.load(r#"
        function (fn)
            local upvalues = {};
            local f_info = debug.getinfo(fn, "u");
            for i = 1,f_info.nups do
                local up_name, up_val = debug.getupvalue(fn, i);
                if up_name == "_ENV" then
                    table.insert(upvalues, {"_ENV", nil});
                else
                    table.insert(upvalues, {up_name, up_val});
                end
            end
            return upvalues;
        end
    "#).eval()?;
    let f_upvalues: Vec<mlua::Value> = get_upvalues.call(func.clone())?;

    let mut history_with_f = history.clone();
    history_with_f.insert(func.to_pointer());

    // let upvalues = detach_value(mlua::Value::Table(f_upvalues), lua, history_with_f)?;
    let upvalues_res: Result<Vec<DetachedLuaValue>, mlua::Error> = f_upvalues.into_iter().map(|v| detach_value(v, lua, &history_with_f)).collect();
    let upvalues = upvalues_res?;

    Ok(FunctionDump { source, upvalues })
}

pub fn hydrate_function<'lua>(func: FunctionDump, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Function<'lua>> {
    let (source, upvalues) = (func.source, func.upvalues);

    let hydrate: mlua::Function = lua.load(r#"
        function (source, upvalues)
            local fn = load(source);
            for i, v in ipairs(upvalues) do
                local up_name, up_value = table.unpack(v);
                if up_name ~= "_ENV" then 
                    debug.setupvalue(fn, i, up_value);
                end
            end
            return fn
        end
    "#).eval()?;
    let hydrated_func: mlua::Function = hydrate.call((lua.create_string(&source[..])?, upvalues))?;

    Ok(hydrated_func)
}

pub fn detach_value<'lua>(value: mlua::Value<'lua>, lua: &'lua mlua::Lua, history: &HashSet<*const c_void>) -> Result<DetachedLuaValue, mlua::Error> {
    match value {
        mlua::Value::Nil => Ok(DetachedLuaValue::Nil),
        mlua::Value::Boolean(v) => Ok(DetachedLuaValue::Boolean(v)),
        mlua::Value::Integer(v) => Ok(DetachedLuaValue::Integer(v)),
        mlua::Value::Number(v) => Ok(DetachedLuaValue::Number(v)),
        mlua::Value::String(v) => Ok(DetachedLuaValue::String(String::from(v.to_str()?))),
        mlua::Value::Table(t) => {
            if history.contains(&t.to_pointer()) {
                Err(mlua::Error::runtime(format!("Cycle encountered when extracting Table: {:?}", t)))
            } else {
                let mut history_with_t = history.clone();
                history_with_t.insert(t.to_pointer());
    
                let mut detached_map: HashMap<DetachedLuaValue, DetachedLuaValue> = HashMap::new();
                for pair in t.pairs::<mlua::Value, mlua::Value>().into_iter() {
                    match pair {
                        Err(e) => return Err(e),
                        Ok((k, v)) => {
                            let k_detached = detach_value(k, lua, &history_with_t)?;
                            let v_detached = detach_value(v, lua, &history_with_t)?;
                            detached_map.insert(k_detached, v_detached);
                        }
                    }
                }
    
                Ok(DetachedLuaValue::Table(detached_map))
            }
        },
        mlua::Value::Function(f) => Ok(DetachedLuaValue::Function(dump_function(f, lua, &history)?)),
        mlua::Value::UserData(d) => Ok(DetachedLuaValue::UserData(d.to_pointer())),
        mlua::Value::LightUserData(d) => Ok(DetachedLuaValue::LightUserData(d)),
        mlua::Value::Error(e) => Ok(DetachedLuaValue::Error(e)),
        mlua::Value::Thread(t) => Err(mlua::Error::runtime(format!("Cannot serialize a thread object: {:?}", t)))
    }
}

impl <'lua> mlua::FromLua<'lua> for DetachedLuaValue {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        detach_value(value, lua, &HashSet::new())
    }
}

impl <'lua> mlua::IntoLua<'lua> for DetachedLuaValue {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        use DetachedLuaValue::*;
        match self {
            Nil => Ok(mlua::Value::Nil),
            Boolean(val) => Ok(mlua::Value::Boolean(val)),
            LightUserData(val) => Ok(mlua::Value::LightUserData(val)),
            Integer(val) => Ok(mlua::Value::Integer(val)),
            Number(val) => Ok(mlua::Value::Number(val)),
            String(val) => Ok(mlua::Value::String(lua.create_string(val.as_str())?)),
            Table(val) => val.into_lua(lua),
            Function(val) => Ok(mlua::Value::Function(hydrate_function(val, lua)?)),
            UserData(_) => Ok(mlua::Value::Nil),
            Error(val) => Ok(mlua::Value::Error(val))
        }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct FunctionDump {
    pub source: Vec<u8>,
    pub upvalues: Vec<DetachedLuaValue>
}

impl fmt::Display for FunctionDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FnDump(<source>")?;
        
        if self.upvalues.len() > 0 {
            f.write_str(", upvalues=[")?;
            for (i, val) in self.upvalues.iter().enumerate() {
                if i > 0 { f.write_str(", ")?; }
                write!(f, "{}", &val)?;
            }
            f.write_str("]")?;
        }
        f.write_str(")")
    }
}

impl <'lua> mlua::FromLua<'lua> for FunctionDump {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Function(f) => dump_function(f, lua, &HashSet::new()),
            _ => Err(mlua::Error::runtime(format!("Cannot convert non-function value to a FunctionDump: {:?}", value)))
        }
    }
}

impl <'lua> mlua::IntoLua<'lua> for FunctionDump {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        hydrate_function(self, lua).map(|f| mlua::Value::Function(f))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    use crate::lua::lua_env::create_lua_env;

    #[test]
    fn test_extract_function_with_upvalues() {
        let lua = create_lua_env(Path::new(".")).unwrap();

        let add_five_func: mlua::Function = lua.load(r#"
            local x = 5;
            return function(y)
                return x + y
            end
        "#).call(()).unwrap();

        let dumped_add_five_func: FunctionDump = lua.unpack(mlua::Value::Function(add_five_func)).unwrap();

        let lua_2 = create_lua_env(Path::new(".")).unwrap();
        let add_five_func_2 = hydrate_function(dumped_add_five_func, &lua_2).unwrap();
        let result: i32 = add_five_func_2.call(3).unwrap();
        assert_eq!(result, 8);
    }
}