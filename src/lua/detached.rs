extern crate serde_json;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::os::raw::c_void;
use std::sync::{Arc, RwLock};

use crate::lua::userdata::CobbleUserData;
use crate::util::onscopeexit::OnScopeExitMut;

#[derive(Clone)]
pub enum DetachedLuaValue {
    Nil,
    Boolean(bool),
    Integer(mlua::Integer),
    Number(mlua::Number),
    String(String),
    Table(Arc<RwLock<(HashMap<DetachedLuaValue, DetachedLuaValue>, Option<DetachedLuaValue>)>>),
    Function(Arc<RwLock<FunctionDump>>),
    UserData(CobbleUserData)
}

impl DetachedLuaValue {
    pub fn to_json(&self) -> serde_json::Value {
        use DetachedLuaValue::*;
        match self {
            Nil => serde_json::Value::Null,
            Boolean(b) => serde_json::Value::Bool(*b),
            Integer(i) => serde_json::Number::from_f64(*i as f64)
                .map(|n| serde_json::Value::Number(n))
                .unwrap_or(serde_json::Value::Null),
            Number(f) => serde_json::Number::from_f64(*f)
                .map(|n| serde_json::Value::Number(n))
                .unwrap_or(serde_json::Value::Null),
            String(s) => serde_json::Value::String(s.clone()),
            Table(tbl) => {
                let tbl_lock = tbl.read().unwrap();
                let (t, _meta) = &*tbl_lock;
                let mut map: serde_json::Map<std::string::String, serde_json::Value> =
                    serde_json::Map::with_capacity(t.len());
                for (k, v) in t.iter() {
                    let k_json = match k {
                        String(s) => s.clone(),
                        _ => format!("{}", k),
                    };
                    map.insert(k_json, v.to_json());
                }
                serde_json::Value::Object(map)
            }
            Function(f) => serde_json::Value::String(format!("{}", &*f.read().unwrap())),
            UserData(d) => serde_json::Value::String(format!("{}", d)),
        }
    }
}

fn fmt_debug_detached_value_with_history(value: &DetachedLuaValue, history: &mut HashSet<DetachedLuaValue>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if history.contains(value) {
        write!(f, "...")?;
        return Ok(());
    }

    history.insert(value.clone());
    let val_clone = value.clone();
    let mut history = OnScopeExitMut::new(history, Box::new(move |hist| { hist.remove(&val_clone); }));

    use DetachedLuaValue::*;
    match value {
        Nil => f.write_str("nil"),
        Boolean(val) => f.write_str(if *val { "true" } else { "false" }),
        Integer(val) => write!(f, "{}", val),
        Number(val) => write!(f, "{}", val),
        String(val) => write!(f, "\"{}\"", val.as_str()),
        Table(tbl) => {
            let tbl_lock = tbl.read().unwrap();
            let (t, _meta) = &*tbl_lock;
            f.write_str("{")?;
            for (i, (k, v)) in t.iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                fmt_debug_detached_value_with_history(k, history.as_mut(), f)?;
                write!(f, ": ")?;
                fmt_debug_detached_value_with_history(v, history.as_mut(), f)?;
            }
            f.write_str("}")
        }
        Function(val) => {
            let func = val.read().unwrap();
            fmt_debug_function_dump_with_history(&*func, history.as_mut(), f)
        }
        UserData(val) => write!(f, "{}", val),
    }
}

impl fmt::Debug for DetachedLuaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_debug_detached_value_with_history(self, &mut HashSet::new(), f)
    }
}

impl fmt::Display for DetachedLuaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_debug_detached_value_with_history(self, &mut HashSet::new(), f)
    }
}

impl Eq for DetachedLuaValue {}
impl PartialEq for DetachedLuaValue {
    fn eq(&self, other: &Self) -> bool {
        use DetachedLuaValue::*;
        match self {
            Nil => match other {
                Nil => true,
                _ => false,
            },
            Boolean(self_val) => match other {
                Boolean(other_val) => self_val == other_val,
                _ => false,
            },
            Integer(this_val) => match other {
                Integer(other_val) => this_val == other_val,
                _ => false,
            },
            Number(this_val) => match other {
                Number(other_val) => this_val.to_bits() == other_val.to_bits(),
                _ => false,
            },
            String(this_val) => match other {
                String(other_val) => this_val == other_val,
                _ => false,
            },
            Table(this_val) => match other {
                Table(other_val) => Arc::as_ptr(&this_val) == Arc::as_ptr(other_val),
                _ => false,
            },
            Function(this_val) => match other {
                Function(other_val) => Arc::as_ptr(this_val) == Arc::as_ptr(other_val),
                _ => false,
            },
            UserData(this_val) => match other {
                UserData(other_val) => this_val == other_val,
                _ => false,
            },
        }
    }
}

impl Hash for DetachedLuaValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use DetachedLuaValue::*;
        match self {
            Nil => {
                state.write(b"nil");
            }
            Boolean(val) => {
                state.write(b"bln");
                val.hash(state);
            }
            Integer(val) => {
                state.write(b"int");
                val.hash(state);
            }
            Number(val) => {
                state.write(b"num");
                val.to_bits().hash(state);
            }
            String(val) => {
                state.write(b"str");
                val.hash(state);
            }
            Table(tbl) => {
                state.write(b"tbl");
                Arc::as_ptr(tbl).hash(state);
            }
            Function(f) => {
                state.write(b"fun");
                Arc::as_ptr(f).hash(state);
            }
            UserData(d) => {
                state.write(b"usr");
                d.hash(state);
            }
        }
    }
}

pub fn dump_function<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>,
    val_map: &mut HashMap<*const c_void, DetachedLuaValue>,
    value_references: &mut Vec<mlua::Value<'lua>>
) -> mlua::Result<Arc<RwLock<FunctionDump>>> {
    if func.info().what != "Lua" {
        return Err(mlua::Error::runtime(format!(
            "Cannot serialize a function that is not a pure Lua function: {:?}",
            &func
        )));
    }

    let source = dump_function_source(lua, func.clone())?;
    
    let function_dump = Arc::new(RwLock::new(FunctionDump {
        source,
        upvalues: Vec::new()
    }));

    val_map.insert(func.to_pointer(), DetachedLuaValue::Function(function_dump.clone()));

    let upvalues = dump_function_upvalues(lua, func, val_map, value_references)?;

    {
        let mut function_dump_lock = function_dump.write().unwrap();
        function_dump_lock.upvalues = upvalues;
    }

    Ok(function_dump)
}

pub fn dump_function_source<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>
) -> mlua::Result<Vec<u8>> {
    if func.info().what != "Lua" {
        return Err(mlua::Error::runtime(format!(
            "Cannot serialize a function that is not a pure Lua function: {:?}",
            &func
        )));
    }

    let func_source_dump: mlua::String = lua.load("return string.dump(...)").call(func)?;
    Ok(func_source_dump.as_bytes().to_owned())
}

pub fn dump_function_upvalues<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>,
    val_map: &mut HashMap<*const c_void, DetachedLuaValue>,
    value_references: &mut Vec<mlua::Value<'lua>>
) -> mlua::Result<Vec<DetachedLuaValue>> {
    let get_upvalues: mlua::Function = lua
        .load(
            r#"
        function (fn)
            local upvalues = {};
            local f_info = debug.getinfo(fn, "u");
            for i = 1,f_info.nups do
                local up_name, up_val = debug.getupvalue(fn, i);
                if up_name == "_ENV" then
                    upvalues[i] = {"_ENV", nil};
                else
                    upvalues[i] = {up_name, up_val};
                end
            end
            return upvalues;
        end
    "#,
        )
        .eval()?;
    let f_upvalues: Vec<mlua::Value> = get_upvalues.call(func.clone())?;

    // let upvalues = detach_value(mlua::Value::Table(f_upvalues), lua, history_with_f)?;
    let upvalues_res: mlua::Result<Vec<DetachedLuaValue>> = f_upvalues
        .into_iter()
        .map(|v| detach_value(lua, v, val_map, value_references))
        .collect();
    let upvalues = upvalues_res?;

    Ok(upvalues)
}

pub fn hydrate_function_upvalues<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>,
    upvalues: &Vec<DetachedLuaValue>,
    val_map: &mut HashMap<*const c_void, mlua::Value<'lua>>
) -> mlua::Result<()> {
    let upvalues_table = lua.create_table()?;
    for upval in upvalues {
        upvalues_table.push(hydrate_value(lua, upval, val_map)?)?;
    }

    let hydrate: mlua::Function = lua
        .load(
            r#"
        function (fn, upvalues)
            for i, v in ipairs(upvalues) do
                local up_name, up_value = table.unpack(v);
                if up_name == "_ENV" then
                    debug.setupvalue(fn, i, _ENV)
                else 
                    debug.setupvalue(fn, i, up_value);
                end
            end
            return fn
        end
    "#,
        )
        .eval()?;
    hydrate.call((func, upvalues_table))?;

    Ok(())
}

pub fn dump_table<'lua>(
    lua: &'lua mlua::Lua,
    value: mlua::Table<'lua>,
    val_map: &mut HashMap<*const c_void, DetachedLuaValue>,
    value_references: &mut Vec<mlua::Value<'lua>>
) -> mlua::Result<HashMap<DetachedLuaValue, DetachedLuaValue>> {
    let mut detached_map: HashMap<DetachedLuaValue, DetachedLuaValue> = HashMap::new();
    for pair in value.pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;

        let k_detached = detach_value(lua, k, val_map, value_references)?;
        let v_detached = detach_value(lua, v, val_map, value_references)?;
        detached_map.insert(k_detached, v_detached);
    }

    Ok(detached_map)
}

pub fn hydrate_table<'lua>(
    lua: &'lua mlua::Lua,
    table: &HashMap<DetachedLuaValue, DetachedLuaValue>,
    dest_table: &mlua::Table,
    val_map: &mut HashMap<*const c_void, mlua::Value<'lua>>
) -> mlua::Result<()> {
    for (k, v) in table {
        let lua_k = hydrate_value(lua, k, val_map)?;
        let lua_v = hydrate_value(lua, v, val_map)?;

        dest_table.set(lua_k, lua_v)?;
    }

    Ok(())
}

pub fn detach_value<'lua>(
    lua: &'lua mlua::Lua,
    value: mlua::Value<'lua>,
    val_map: &mut HashMap<*const c_void, DetachedLuaValue>,
    value_references: &mut Vec<mlua::Value<'lua>>
) -> mlua::Result<DetachedLuaValue> {
    match value {
        mlua::Value::Nil => Ok(DetachedLuaValue::Nil),
        mlua::Value::Boolean(v) => Ok(DetachedLuaValue::Boolean(v)),
        mlua::Value::Integer(v) => Ok(DetachedLuaValue::Integer(v)),
        mlua::Value::Number(v) => Ok(DetachedLuaValue::Number(v)),
        mlua::Value::String(v) => Ok(DetachedLuaValue::String(String::from(v.to_str()?))),
        mlua::Value::Table(t) => {
            let lua_val_ptr = t.to_pointer();
            match val_map.get(&lua_val_ptr) {
                Some(val) => Ok(val.clone()),
                None => {
                    value_references.push(mlua::Value::Table(t.clone()));

                    let table = Arc::new(RwLock::new((HashMap::new(), None)));
                    val_map.insert(lua_val_ptr, DetachedLuaValue::Table(table.clone()));
        
                    let meta = match t.get_metatable() {
                        Some(metatable) => Some(detach_value(lua, mlua::Value::Table(metatable), val_map, value_references)?),
                        None => None,
                    };
        
                    let detached_map = dump_table(lua, t, val_map, value_references)?;
        
                    {
                        let mut table_lock = table.write().unwrap();
                        let (tbl, meta_tbl) = &mut *table_lock;
                        *tbl = detached_map;
                        *meta_tbl = meta;
                    }
        
                    Ok(DetachedLuaValue::Table(table))
                }
            }
        }
        mlua::Value::Function(f) => {
            let lua_val_ptr = f.to_pointer();
            match val_map.get(&lua_val_ptr) {
                Some(val) => Ok(val.clone()),
                None => {
                    value_references.push(mlua::Value::Function(f.clone()));
                    Ok(DetachedLuaValue::Function(dump_function(lua, f, val_map, value_references)?))
                }
            }
        }
        mlua::Value::UserData(d) => Ok(DetachedLuaValue::UserData(CobbleUserData::from_userdata(
            lua, d,
        )?)),
        mlua::Value::LightUserData(d) => Err(mlua::Error::runtime(format!(
            "Cannot serialize a light user data object: {:?}",
            d
        ))),
        mlua::Value::Error(e) => Err(mlua::Error::runtime(format!(
            "Cannot serialize an error object: {:?}",
            e
        ))),
        mlua::Value::Thread(t) => Err(mlua::Error::runtime(format!(
            "Cannot serialize a thread object: {:?}",
            t
        ))),
    }
}

pub fn hydrate_value<'lua>(
    lua: &'lua mlua::Lua,
    value: &DetachedLuaValue,
    val_map: &mut HashMap<*const c_void, mlua::Value<'lua>>
) -> mlua::Result<mlua::Value<'lua>> {
    match value {
        DetachedLuaValue::Nil => Ok(mlua::Value::Nil),
        DetachedLuaValue::Boolean(b) => Ok(mlua::Value::Boolean(*b)),
        DetachedLuaValue::Integer(i) => Ok(mlua::Value::Integer(*i)),
        DetachedLuaValue::Number(n) => Ok(mlua::Value::Number(*n)),
        DetachedLuaValue::String(s) => Ok(mlua::Value::String(lua.create_string(s)?)),
        DetachedLuaValue::Table(table) => {
            let d_val_ptr = Arc::as_ptr(table) as *const c_void;
            match val_map.get(&d_val_ptr) {
                Some(val) => Ok(val.clone()),
                None => {
                    let lua_table = lua.create_table()?;
                    val_map.insert(d_val_ptr, mlua::Value::Table(lua_table.clone()));

                    let table_lock = table.read().unwrap();
                    let (tbl, meta) = &*table_lock;

                    hydrate_table(lua, tbl, &lua_table, val_map)?;
        
                    let lua_metatable = match meta {
                        Some(m) => match hydrate_value(lua, m, val_map)? {
                            mlua::Value::Table(t) => Some(t),
                            _ => None
                        },
                        None => None
                    };
                    lua_table.set_metatable(lua_metatable);
                    Ok(mlua::Value::Table(lua_table))
                }
            }
        }
        DetachedLuaValue::Function(f) => {
            let d_val_ptr = Arc::as_ptr(f) as *const c_void;
            match val_map.get(&d_val_ptr) {
                Some(val) => Ok(val.clone()),
                None => {
                    let f_lock = f.read().unwrap();
                    let lua_source_str = lua.create_string(&f_lock.source)?;
                    let lua_func: mlua::Function = lua.load("return load(...)").call(lua_source_str)?;
                    val_map.insert(d_val_ptr, mlua::Value::Function(lua_func.clone()));
        
                    hydrate_function_upvalues(lua, lua_func.clone(), &f_lock.upvalues, val_map)?;
        
                    Ok(mlua::Value::Function(lua_func))
                }
            }
        }
        DetachedLuaValue::UserData(d) => Ok(mlua::Value::UserData(d.to_userdata(lua)?)),
    }
}

impl<'lua> mlua::FromLua<'lua> for DetachedLuaValue {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        detach_value(lua, value, &mut HashMap::new(), &mut Vec::new())
    }
}

impl<'lua> mlua::IntoLua<'lua> for DetachedLuaValue {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        hydrate_value(lua, &self, &mut HashMap::new())
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct FunctionDump {
    pub source: Vec<u8>,
    pub upvalues: Vec<DetachedLuaValue>,
}

fn fmt_debug_function_dump_with_history(func: &FunctionDump, history: &mut HashSet<DetachedLuaValue>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "function(source=")?;
    for b in func.source.iter() { write!(f, "{:x}", b)?; }
    write!(f, ", upvalues=[")?;
    for (i, up) in func.upvalues.iter().enumerate() {
        if i > 0 { write!(f, ", ")?; }
        fmt_debug_detached_value_with_history(up, history, f)?;
    }
    write!(f, "])")
}

impl fmt::Debug for FunctionDump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_debug_function_dump_with_history(self, &mut HashSet::new(), f)
    }
}

impl fmt::Display for FunctionDump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_debug_function_dump_with_history(self, &mut HashSet::new(), f)
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

        let add_five_func: mlua::Function = lua
            .load(
                r#"
            local x = 5;
            return function(y)
                return x + y
            end
        "#,
            )
            .call(())
            .unwrap();

        let dumped_add_five_func = detach_value(&lua, mlua::Value::Function(add_five_func), &mut HashMap::new(), &mut Vec::new()).unwrap();

        let lua_2 = create_lua_env(Path::new(".")).unwrap();
        let add_five_func_2: mlua::Value = lua_2.pack(dumped_add_five_func).unwrap();
        let result: i32 = add_five_func_2.as_function().unwrap().call(3).unwrap();
        assert_eq!(result, 8);
    }
}
