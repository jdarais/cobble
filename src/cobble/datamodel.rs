extern crate mlua;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::os::raw::c_void;

use mlua::IntoLua;

#[derive(Clone)]
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

impl Eq for DetachedLuaValue {}
impl PartialEq for DetachedLuaValue {
    fn eq(&self, other: &Self) -> bool {
        match self {
            DetachedLuaValue::Nil => match other {
                DetachedLuaValue::Nil => true,
                _ => false
            },
            DetachedLuaValue::Boolean(self_val) => match other {
                DetachedLuaValue::Boolean(other_val) => self_val == other_val,
                _ => false
            },
            DetachedLuaValue::LightUserData(this_val) => match other {
                DetachedLuaValue::LightUserData(other_val) => this_val.0 == other_val.0,
                _ => false
            },
            DetachedLuaValue::Integer(this_val) => match other {
                DetachedLuaValue::Integer(other_val) => this_val == other_val,
                _ => false
            },
            DetachedLuaValue::Number(this_val) => match other {
                DetachedLuaValue::Number(other_val) => this_val.to_bits() == other_val.to_bits(),
                _ => false
            },
            DetachedLuaValue::String(this_val) => match other {
                DetachedLuaValue::String(other_val) => this_val == other_val,
                _ => false
            },
            DetachedLuaValue::Table(this_val) => match other {
                DetachedLuaValue::Table(other_val) => this_val == other_val,
                _ => false
            },
            DetachedLuaValue::Function(this_val) => match other {
                DetachedLuaValue::Function(other_val) => this_val == other_val,
                _ => false
            },
            DetachedLuaValue::UserData(this_val) => match other {
                DetachedLuaValue::UserData(other_val) => this_val == other_val,
                _ => false
            },
            DetachedLuaValue::Error(this_val) => match other {
                DetachedLuaValue::Error(other_val) => this_val.to_string() == other_val.to_string(),
                _ => false
            }
        }
    }
}

impl Hash for DetachedLuaValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            DetachedLuaValue::Nil => { state.write(b"nil"); },
            DetachedLuaValue::Boolean(val) => {
                state.write(b"bln");
                val.hash(state);
            },
            DetachedLuaValue::LightUserData(val) => {
                state.write(b"lud");
                val.0.hash(state);
            },
            DetachedLuaValue::Integer(val) => {
                state.write(b"int");
                val.hash(state);
            },
            DetachedLuaValue::Number(val) => {
                state.write(b"num");
                val.to_bits().hash(state);
            },
            DetachedLuaValue::String(val) => {
                state.write(b"str");
                val.hash(state);
            },
            DetachedLuaValue::Table(tbl) => {
                state.write(b"tbl");
                for (k, v) in tbl.iter() {
                    state.write(b"key");
                    k.hash(state);
                    state.write(b"val");
                    v.hash(state);
                }
            },
            DetachedLuaValue::Function(f) => {
                state.write(&f.source[..]);
                for upval in f.upvalues.iter() {
                    upval.hash(state);
                }
            },
            DetachedLuaValue::UserData(val) => {
                state.write(b"usd");
                val.hash(state);
            },
            DetachedLuaValue::Error(val) => {
                state.write(b"err");
                val.to_string().hash(state);
            }
        }

    }
}

pub fn dump_function<'lua>(func: mlua::Function<'lua>, lua: &'lua mlua::Lua, history: &HashSet<*const c_void>) -> Result<FunctionDump, mlua::Error> {
    if history.contains(&func.to_pointer()) {
        return Err(mlua::Error::RuntimeError(format!("Cycle encountered when extracting Function: {:?}", &func)));
    }

    if func.info().what != "Lua" {
        return Err(mlua::Error::RuntimeError(format!("Cannot serialize a function that is not a pure Lua function: {:?}", &func)));
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
                Err(mlua::Error::RuntimeError(format!("Cycle encountered when extracting Table: {:?}", t)))
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
        mlua::Value::Thread(t) => Err(mlua::Error::RuntimeError(format!("Cannot serialize a thread object: {:?}", t)))
    }
}

impl <'lua> mlua::FromLua<'lua> for DetachedLuaValue {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        detach_value(value, lua, &HashSet::new())
    }
}

impl <'lua> mlua::IntoLua<'lua> for DetachedLuaValue {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        match self {
            DetachedLuaValue::Nil => Ok(mlua::Value::Nil),
            DetachedLuaValue::Boolean(val) => Ok(mlua::Value::Boolean(val)),
            DetachedLuaValue::LightUserData(val) => Ok(mlua::Value::LightUserData((val))),
            DetachedLuaValue::Integer(val) => Ok(mlua::Value::Integer(val)),
            DetachedLuaValue::Number(val) => Ok(mlua::Value::Number(val)),
            DetachedLuaValue::String(val) => Ok(mlua::Value::String(lua.create_string(val.as_str())?)),
            DetachedLuaValue::Table(val) => val.into_lua(lua),
            DetachedLuaValue::Function(val) => Ok(mlua::Value::Function(hydrate_function(val, lua)?)),
            DetachedLuaValue::UserData(_) => Ok(mlua::Value::Nil),
            DetachedLuaValue::Error(val) => Ok(mlua::Value::Error(val))
        }
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct FunctionDump {
    pub source: Vec<u8>,
    pub upvalues: Vec<DetachedLuaValue>
}

impl <'lua> mlua::FromLua<'lua> for FunctionDump {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Function(f) => dump_function(f, lua, &HashSet::new()),
            _ => Err(mlua::Error::RuntimeError(format!("Cannot convert non-function value to a FunctionDump: {:?}", value)))
        }
    }
}

pub enum Action {
    Cmd(Vec<String>),
    Func(FunctionDump)
}

impl <'lua> mlua::FromLua<'lua> for Action {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Function(func) => Ok(Action::Func(dump_function(func, lua, &HashSet::new())?)),
            mlua::Value::Table(tbl) => {
                let cmd_args_res: mlua::Result<Vec<String>> = tbl.sequence_values().collect();
                let cmd_args = cmd_args_res?;
                Ok(Action::Cmd(cmd_args))
            },
            val => Err(mlua::Error::RuntimeError(format!("Unable to convert value to Action: {:?}", val)))
        }
    }
}

pub struct DependencyList(Vec<Dependency>);

impl <'lua> mlua::FromLua<'lua> for DependencyList {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let deps_table: mlua::Table = lua.unpack(value)?;
        let file_deps: Option<Vec<String>> = deps_table.get("files")?;
        let task_deps: Option<Vec<String>> = deps_table.get("tasks")?;
        let deps: Vec<Dependency> = file_deps.unwrap_or(vec![]).into_iter().map(|f| Dependency::File(f))
            .chain(task_deps.unwrap_or(vec![]).into_iter().map(|t| Dependency::Task(t)))
            .collect();

        Ok(DependencyList(deps))
    }
}

pub enum Dependency {
    File(String),
    Task(String)
}

pub struct Artifact {
    pub filename: String
}

impl <'lua> mlua::FromLua<'lua> for Artifact {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let filename: String = lua.unpack(value)?;

        Ok(Artifact{ filename })
    }
}

pub struct TaskDef {
    pub build_env: String,
    pub actions: Vec<Action>,
    pub deps: Vec<Dependency>,
    pub artifacts: Vec<Artifact>
}

impl <'lua> mlua::FromLua<'lua> for TaskDef {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let task_def_table: mlua::Table = lua.unpack(value)?;
        let build_env: String = task_def_table.get("build_env")?;
        let actions: Vec<Action> = task_def_table.get("actions")?;
        let deps: Vec<Dependency> = task_def_table.get::<_, DependencyList>("deps")?.0;
        let artifacts: Vec<Artifact> = task_def_table.get("artifacts")?;

        Ok(TaskDef {
            build_env,
            actions,
            deps,
            artifacts
        })
    }
}

pub struct Task {
    pub name: String,
    pub build_env: String,
    pub actions: Vec<Action>,
    pub deps: Vec<Dependency>,
    pub artifacts: Vec<Artifact>
}

impl <'lua> mlua::FromLua<'lua> for Task {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let task_table = lua.unpack::<mlua::Table>(value)?;
        let name: String = task_table.get("name")?;
        let build_env: String = task_table.get("build_env")?;
        let actions: Vec<Action> = task_table.get("actions")?;
        let deps: Vec<Dependency> = task_table.get::<_, DependencyList>("deps")?.0;
        let artifacts: Vec<Artifact> = task_table.get("artifacts")?;

        Ok(Task {
            name,
            build_env,
            actions,
            deps,
            artifacts
        })
    }
}

pub struct BuildEnv {
    pub name: String,
    pub install_actions: Vec<Action>,
    pub install_deps: Vec<Dependency>,
    pub exec: FunctionDump,
}

impl <'lua> mlua::FromLua<'lua> for BuildEnv {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        let build_env_def_table = match value {
            mlua::Value::Table(tbl) => tbl,
            val => { return Err(mlua::Error::RuntimeError(format!("Unable to convert value to a BuildEnvDef: {:?}", val))); }
        };

        let name: String = build_env_def_table.get("name")?;

        let install_actions: Vec<Action> = build_env_def_table.get("install_actions")?;
        let install_deps: Vec<Dependency> = build_env_def_table.get::<_, DependencyList>("install_deps")?.0;
        let exec_func: mlua::Function<'lua> = build_env_def_table.get("exec")?;

        let exec = dump_function(exec_func, lua, &HashSet::new())?;

        Ok(BuildEnv {
            name,
            install_actions,
            install_deps,
            exec
        })
    }
}


#[cfg(test)]
mod tests {
    extern crate mlua;

    use super::*;

    use crate::cobble::lua_env::create_lua_env;

    #[test]
    fn test_extract_function_with_upvalues() {
        let lua = create_lua_env().unwrap();

        let add_five_func: mlua::Function = lua.load(r#"
            local x = 5;
            return function(y)
                return x + y
            end
        "#).call(()).unwrap();

        let dumped_add_five_func: FunctionDump = lua.unpack(mlua::Value::Function(add_five_func)).unwrap();

        let lua_2 = create_lua_env().unwrap();
        let add_five_func_2 = hydrate_function(dumped_add_five_func, &lua_2).unwrap();
        let result: i32 = add_five_func_2.call(3).unwrap();
        assert_eq!(result, 8);
    }

    #[test]
    fn test_build_env_def_from_lua_table() {
        let lua = create_lua_env().unwrap();

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