use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::os::raw::c_void;

use mlua::IntoLua;
use serde::{Deserialize, Serialize};

use crate::lua::userdata::CobbleUserData;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SerLuaValueBlock {
    pub values: Vec<SerLuaValue>,
}

impl fmt::Display for SerLuaValueBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        refify_ser_lua_value(0, &self.values).fmt(f)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SerLuaTable {
    pub entries: Vec<(usize, usize)>,
    pub metatable: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SerLuaFunction {
    pub source: Vec<u8>,
    pub upvalues: Vec<(String, usize)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SerLuaValue {
    Nil,
    Boolean(bool),
    Integer(mlua::Integer),
    Number(mlua::Number),
    String(String),
    Table(SerLuaTable),
    Function(SerLuaFunction),
    UserData(CobbleUserData),
}

pub struct SerLuaTableRef<'a> {
    table: &'a SerLuaTable,
    index: usize,
    ref_values: &'a Vec<SerLuaValue>,
}

impl<'a> SerLuaTableRef<'a> {
    pub fn entries(&self) -> impl Iterator<Item = (SerLuaValueRef<'a>, SerLuaValueRef<'a>)> {
        self.table.entries.iter().map(|(k, v)| {
            let k_ref = refify_ser_lua_value(*k, self.ref_values);
            let v_ref = refify_ser_lua_value(*v, self.ref_values);
            (k_ref, v_ref)
        })
    }

    pub fn metatable(&self) -> Option<SerLuaValueRef<'a>> {
        self.table
            .metatable
            .map(|v| refify_ser_lua_value(v, self.ref_values))
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

impl<'a> Eq for SerLuaTableRef<'a> {}

impl<'a> PartialEq for SerLuaTableRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> PartialOrd for SerLuaTableRef<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for SerLuaTableRef<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        let metatable_cmp = self.metatable().cmp(&other.metatable());
        match metatable_cmp {
            Ordering::Equal => {}
            _ => {
                return metatable_cmp;
            }
        }

        self.entries().cmp(other.entries())
    }
}

pub struct SerLuaFunctionRef<'a> {
    func: &'a SerLuaFunction,
    index: usize,
    ref_values: &'a Vec<SerLuaValue>,
}

impl<'a> SerLuaFunctionRef<'a> {
    pub fn source(&self) -> &Vec<u8> {
        &self.func.source
    }

    pub fn upvalues(&self) -> impl Iterator<Item = (&'a str, SerLuaValueRef<'a>)> {
        self.func.upvalues.iter().map(|(k, v)| {
            let k_str = k.as_str();
            let v_ref = refify_ser_lua_value(*v, self.ref_values);
            (k_str, v_ref)
        })
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

impl<'a> Eq for SerLuaFunctionRef<'a> {}

impl<'a> PartialEq for SerLuaFunctionRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> PartialOrd for SerLuaFunctionRef<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for SerLuaFunctionRef<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        let metatable_cmp = self.source().cmp(&other.source());
        match metatable_cmp {
            Ordering::Equal => {}
            _ => {
                return metatable_cmp;
            }
        }

        self.upvalues().cmp(other.upvalues())
    }
}

pub enum SerLuaValueRef<'a> {
    Nil,
    Boolean(bool),
    Integer(mlua::Integer),
    Number(mlua::Number),
    String(&'a str),
    Table(SerLuaTableRef<'a>),
    Function(SerLuaFunctionRef<'a>),
    UserData(&'a CobbleUserData),
}

impl<'a> SerLuaValueRef<'a> {
    pub fn is_nil(&self) -> bool {
        match self {
            SerLuaValueRef::Nil => true,
            _ => false,
        }
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            SerLuaValueRef::Boolean(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<mlua::Integer> {
        match self {
            SerLuaValueRef::Integer(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<mlua::Number> {
        match self {
            SerLuaValueRef::Number(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&'a str> {
        match self {
            SerLuaValueRef::String(s) => Some(*s),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&SerLuaTableRef<'a>> {
        match self {
            SerLuaValueRef::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<&SerLuaFunctionRef<'a>> {
        match self {
            SerLuaValueRef::Function(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_userdata(&self) -> Option<&'a CobbleUserData> {
        match self {
            SerLuaValueRef::UserData(d) => Some(*d),
            _ => None,
        }
    }
}

impl<'a> Eq for SerLuaValueRef<'a> {}

impl<'a> PartialEq for SerLuaValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> PartialOrd for SerLuaValueRef<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for SerLuaValueRef<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        use SerLuaValueRef::*;
        match self {
            Nil => match other {
                Nil => Ordering::Equal,
                _ => Ordering::Less,
            },
            Boolean(v) => match other {
                Nil => Ordering::Greater,
                Boolean(ov) => bool::cmp(v, ov),
                _ => Ordering::Less,
            },
            Integer(v) => match other {
                Nil | Boolean(_) => Ordering::Greater,
                Integer(ov) => i64::cmp(v, ov),
                _ => Ordering::Less,
            },
            Number(v) => match other {
                Nil | Boolean(_) | Integer(_) => Ordering::Greater,
                Number(ov) => f64::total_cmp(v, ov),
                _ => Ordering::Less,
            },
            String(s) => match other {
                Nil | Boolean(_) | Integer(_) | Number(_) => Ordering::Greater,
                String(os) => str::cmp(s, os),
                _ => Ordering::Less,
            },
            Table(t) => match other {
                Nil | Boolean(_) | Integer(_) | Number(_) | String(_) => Ordering::Greater,
                Table(ot) => t.cmp(ot),
                _ => Ordering::Less,
            },
            Function(f) => match other {
                Nil | Boolean(_) | Integer(_) | Number(_) | String(_) | Table(_) => {
                    Ordering::Greater
                }
                Function(of) => f.cmp(of),
                _ => Ordering::Less,
            },
            UserData(d) => match other {
                Nil | Boolean(_) | Integer(_) | Number(_) | String(_) | Table(_) | Function(_) => {
                    Ordering::Greater
                }
                UserData(od) => d.cmp(od),
            },
        }
    }
}

fn fmt_ser_lua_value_ref_with_history<'a>(
    f: &mut fmt::Formatter,
    value: &SerLuaValueRef<'a>,
    history: &mut HashSet<usize>,
) -> fmt::Result {
    match value {
        SerLuaValueRef::Nil => f.write_str("nil"),
        SerLuaValueRef::Boolean(v) => write!(f, "{}", v),
        SerLuaValueRef::Integer(v) => write!(f, "{}", v),
        SerLuaValueRef::Number(v) => write!(f, "{}", v),
        SerLuaValueRef::String(s) => write!(f, "\"{}\"", s),
        SerLuaValueRef::Table(t) => {
            let index = t.index();
            if history.contains(&index) {
                return f.write_str("<table (cycle)>");
            }

            history.insert(t.index());

            f.write_str("{")?;
            for (i, (k, v)) in t.entries().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }

                f.write_str("[")?;
                fmt_ser_lua_value_ref_with_history(f, &k, history)?;
                f.write_str("]=")?;
                fmt_ser_lua_value_ref_with_history(f, &v, history)?;
            }

            if let Some(mt) = t.metatable() {
                f.write_str("(metatable)=")?;
                fmt_ser_lua_value_ref_with_history(f, &mt, history)?;
            }

            f.write_str("}")
        }
        SerLuaValueRef::Function(_) => f.write_str("<function>"),
        SerLuaValueRef::UserData(d) => write!(f, "<userdata:{}>", d),
    }
}

impl<'a> fmt::Display for SerLuaValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut history: HashSet<usize> = HashSet::new();
        fmt_ser_lua_value_ref_with_history(f, self, &mut history)
    }
}

pub fn refify_ser_lua_value<'a>(
    index: usize,
    ref_values: &'a Vec<SerLuaValue>,
) -> SerLuaValueRef<'a> {
    let val = &ref_values[index];
    match val {
        SerLuaValue::Nil => SerLuaValueRef::Nil,
        SerLuaValue::Boolean(v) => SerLuaValueRef::Boolean(*v),
        SerLuaValue::Integer(v) => SerLuaValueRef::Integer(*v),
        SerLuaValue::Number(v) => SerLuaValueRef::Number(*v),
        SerLuaValue::String(s) => SerLuaValueRef::String(s.as_str()),
        SerLuaValue::Table(t) => SerLuaValueRef::Table(SerLuaTableRef {
            table: t,
            index,
            ref_values,
        }),
        SerLuaValue::Function(f) => SerLuaValueRef::Function(SerLuaFunctionRef {
            func: f,
            index,
            ref_values,
        }),
        SerLuaValue::UserData(d) => SerLuaValueRef::UserData(d),
    }
}

impl<'a> From<&'a SerLuaValueBlock> for SerLuaValueRef<'a> {
    fn from(value: &'a SerLuaValueBlock) -> Self {
        refify_ser_lua_value(0, &value.values)
    }
}

fn dump_function_source<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>,
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
) -> mlua::Result<Vec<mlua::Table<'lua>>> {
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

    get_upvalues.call(func.clone())
}

fn append_ser_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    value: &mlua::Value<'lua>,
    ref_value_index_map: &mut HashMap<*const c_void, (usize, mlua::Value<'lua>)>,
    ref_values: &mut Vec<SerLuaValue>,
) -> mlua::Result<usize> {
    let value_ptr = value.to_pointer();
    if let Some((idx, _)) = ref_value_index_map.get(&value_ptr) {
        return Ok(*idx);
    }

    let ref_index = ref_values.len();
    ref_values.push(SerLuaValue::Nil);
    ref_value_index_map.insert(value.to_pointer(), (ref_index, value.clone()));
    let ser_val = match value {
        mlua::Value::Nil => SerLuaValue::Nil,
        mlua::Value::Boolean(v) => SerLuaValue::Boolean(*v),
        mlua::Value::Integer(v) => SerLuaValue::Integer(*v),
        mlua::Value::Number(v) => SerLuaValue::Number(*v),
        mlua::Value::String(v) => SerLuaValue::String(String::from(v.to_str()?)),
        mlua::Value::Table(t) => {
            ref_values.push(SerLuaValue::Nil);

            let mut entries: Vec<(usize, usize)> = Vec::new();
            for pair in t.clone().pairs() {
                let (k, v): (mlua::Value, mlua::Value) = pair?;
                let k_ref = append_ser_lua_value(lua, &k, ref_value_index_map, ref_values)?;
                let v_ref = append_ser_lua_value(lua, &v, ref_value_index_map, ref_values)?;
                entries.push((k_ref, v_ref));
            }

            entries.sort_by_cached_key(|(k, _v)| refify_ser_lua_value(*k, ref_values));

            let metatable: Option<usize> = match t.get_metatable() {
                Some(mt) => Some(append_ser_lua_value(
                    lua,
                    &mlua::Value::Table(mt),
                    ref_value_index_map,
                    ref_values,
                )?),
                None => None,
            };

            SerLuaValue::Table(SerLuaTable { entries, metatable })
        }
        mlua::Value::Function(f) => {
            let source = dump_function_source(lua, f.clone())?;

            let upvalues_tbl = dump_function_upvalues(lua, f.clone())?;
            let mut upvalues: Vec<(String, usize)> = Vec::with_capacity(upvalues_tbl.len());

            for upval_entry in upvalues_tbl {
                let upval_name: String = upval_entry.get(1)?;
                let upval_value: mlua::Value = upval_entry.get(2)?;

                let upval_value_ref_index =
                    append_ser_lua_value(lua, &upval_value, ref_value_index_map, ref_values)?;

                upvalues.push((upval_name, upval_value_ref_index));
            }

            SerLuaValue::Function(SerLuaFunction { source, upvalues })
        }
        mlua::Value::UserData(d) => {
            SerLuaValue::UserData(CobbleUserData::from_userdata(lua, d.clone())?)
        }
        mlua::Value::LightUserData(d) => {
            return Err(mlua::Error::runtime(format!(
                "Cannot serialize a light user data object: {:?}",
                d
            )));
        }
        mlua::Value::Error(e) => {
            return Err(mlua::Error::runtime(format!(
                "Cannot serialize an error object: {:?}",
                e
            )));
        }
        mlua::Value::Thread(t) => {
            return Err(mlua::Error::runtime(format!(
                "Cannot serialize a thread object: {:?}",
                t
            )));
        }
    };

    ref_values[ref_index] = ser_val;

    Ok(ref_index)
}

pub fn to_ser_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    value: &mlua::Value<'lua>,
) -> mlua::Result<SerLuaValueBlock> {
    let mut ref_value_index_map: HashMap<*const c_void, (usize, mlua::Value<'lua>)> =
        HashMap::new();
    let mut ref_values: Vec<SerLuaValue> = Vec::new();

    append_ser_lua_value(lua, value, &mut ref_value_index_map, &mut ref_values)?;

    Ok(SerLuaValueBlock { values: ref_values })
}

impl<'lua> mlua::FromLua<'lua> for SerLuaValueBlock {
    fn from_lua(value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        to_ser_lua_value(lua, &value)
    }
}

pub fn hydrate_function_upvalues<'lua>(
    lua: &'lua mlua::Lua,
    func: mlua::Function<'lua>,
    upvalues: &Vec<(&str, mlua::Value<'lua>)>,
) -> mlua::Result<()> {
    let upvalues_table = lua.create_table()?;
    for (up_name, up_val) in upvalues {
        let up_tbl = lua.create_table()?;
        up_tbl.push(mlua::Value::String(lua.create_string(up_name)?))?;
        up_tbl.push(up_val)?;

        upvalues_table.push(mlua::Value::Table(up_tbl))?;
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
    hydrate.call::<_, ()>((func, upvalues_table))?;

    Ok(())
}

fn hydrate_ser_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    value_block: &SerLuaValueBlock,
    index: usize,
    index_to_value_map: &mut HashMap<usize, mlua::Value<'lua>>,
) -> mlua::Result<mlua::Value<'lua>> {
    if let Some(val) = index_to_value_map.get(&index) {
        return Ok(val.clone());
    }

    let val = match &value_block.values[index] {
        SerLuaValue::Nil => mlua::Value::Nil,
        SerLuaValue::Boolean(v) => mlua::Value::Boolean(*v),
        SerLuaValue::Integer(v) => mlua::Value::Integer(*v),
        SerLuaValue::Number(v) => mlua::Value::Number(*v),
        SerLuaValue::String(s) => mlua::Value::String(lua.create_string(s)?),
        SerLuaValue::Table(t) => {
            let table = lua.create_table()?;

            // Add the table immediately to the map so it's available for recursive hydrate_ser_lua_value calls
            index_to_value_map.insert(index, mlua::Value::Table(table.clone()));

            for (k, v) in t.entries.iter() {
                let k_val = hydrate_ser_lua_value(lua, value_block, *k, index_to_value_map)?;
                let v_val = hydrate_ser_lua_value(lua, value_block, *v, index_to_value_map)?;

                table.set(k_val, v_val)?;
            }

            if let Some(mt) = &t.metatable {
                let mt_val = hydrate_ser_lua_value(lua, value_block, *mt, index_to_value_map)?;
                match mt_val {
                    mlua::Value::Table(mt_tbl) => {
                        table.set_metatable(Some(mt_tbl));
                    }
                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            "Tried to set metatable to a value that is not a table: {:?}",
                            &mt_val
                        )))
                    }
                };
            }

            mlua::Value::Table(table)
        }
        SerLuaValue::Function(f) => {
            let lua_source_str = lua.create_string(&f.source)?;
            let lua_func: mlua::Function = lua.load("return load(...)").call(lua_source_str)?;

            // Add the table immediately to the map so it's available for recursive hydrate_ser_lua_value calls
            index_to_value_map.insert(index, mlua::Value::Function(lua_func.clone()));

            let mut upvalues: Vec<(&str, mlua::Value)> = Vec::with_capacity(f.upvalues.len());

            for (up_name, up_val) in f.upvalues.iter() {
                let val = hydrate_ser_lua_value(lua, value_block, *up_val, index_to_value_map)?;
                upvalues.push((up_name.as_str(), val));
            }

            hydrate_function_upvalues(lua, lua_func.clone(), &upvalues)?;

            mlua::Value::Function(lua_func)
        }
        SerLuaValue::UserData(d) => mlua::Value::UserData(d.to_userdata(lua)?),
    };

    index_to_value_map.insert(index, val.clone());
    Ok(val)
}

pub fn from_ser_lua_value<'lua>(
    lua: &'lua mlua::Lua,
    block: &SerLuaValueBlock,
    index: usize,
) -> mlua::Result<mlua::Value<'lua>> {
    let mut index_to_value_map: HashMap<usize, mlua::Value> =
        HashMap::with_capacity(block.values.len());
    hydrate_ser_lua_value(lua, block, index, &mut index_to_value_map)
}

impl<'lua> IntoLua<'lua> for SerLuaValueBlock {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        from_ser_lua_value(lua, &self, 0)
    }
}

impl<'lua> IntoLua<'lua> for &SerLuaValueBlock {
    fn into_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        from_ser_lua_value(lua, self, 0)
    }
}

fn append_value_from_block(
    from_block: &SerLuaValueBlock,
    from_index: usize,
    block_index_map: &mut HashMap<usize, usize>,
    to_block: &mut Vec<SerLuaValue>,
) -> usize {
    if let Some(idx) = block_index_map.get(&from_index) {
        return *idx;
    }

    let to_index = to_block.len();
    to_block.push(SerLuaValue::Nil);
    block_index_map.insert(from_index, to_index);

    let to_value = match &from_block.values[from_index] {
        SerLuaValue::Nil => SerLuaValue::Nil,
        SerLuaValue::Boolean(v) => SerLuaValue::Boolean(*v),
        SerLuaValue::Integer(v) => SerLuaValue::Integer(*v),
        SerLuaValue::Number(v) => SerLuaValue::Number(*v),
        SerLuaValue::String(s) => SerLuaValue::String(s.clone()),
        SerLuaValue::Table(t) => {
            let mut entries: Vec<(usize, usize)> = Vec::with_capacity(t.entries.len());

            for (k, v) in t.entries.iter() {
                let to_k = append_value_from_block(from_block, *k, block_index_map, to_block);
                let to_v = append_value_from_block(from_block, *v, block_index_map, to_block);
                entries.push((to_k, to_v))
            }

            let metatable = t
                .metatable
                .as_ref()
                .map(|mt| append_value_from_block(from_block, *mt, block_index_map, to_block));

            SerLuaValue::Table(SerLuaTable { entries, metatable })
        }
        SerLuaValue::Function(f) => {
            let mut upvalues: Vec<(String, usize)> = Vec::with_capacity(f.upvalues.len());

            for (up_name, up_val) in f.upvalues.iter() {
                let to_up_val =
                    append_value_from_block(from_block, *up_val, block_index_map, to_block);
                upvalues.push((up_name.clone(), to_up_val));
            }

            SerLuaValue::Function(SerLuaFunction {
                source: f.source.clone(),
                upvalues,
            })
        }
        SerLuaValue::UserData(d) => SerLuaValue::UserData(d.clone()),
    };

    to_block[to_index] = to_value;
    to_index
}

pub fn extract_lua_value_block(block: &SerLuaValueBlock, index: usize) -> SerLuaValueBlock {
    let mut to_block: Vec<SerLuaValue> = Vec::with_capacity(block.values.len());
    let mut block_index_map: HashMap<usize, usize> = HashMap::with_capacity(block.values.len());

    append_value_from_block(block, index, &mut block_index_map, &mut to_block);

    SerLuaValueBlock { values: to_block }
}
