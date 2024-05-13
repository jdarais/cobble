use std::borrow::Cow;

use crate::util::onscopeexit::OnScopeExitMut;

pub fn prop_path_string(prop_path: &Vec<Cow<'static, str>>) -> String {
    prop_path.join(".")
}

pub fn push_prop_name_if_exists<'a>(
    prop_name: Option<Cow<'static, str>>,
    prop_path: &'a mut Vec<Cow<'static, str>>,
) -> OnScopeExitMut<'a, Vec<Cow<'static, str>>> {
    match prop_name {
        Some(name) => {
            prop_path.push(name);
            OnScopeExitMut::new(
                prop_path,
                Box::new(|path| {
                    path.pop();
                }),
            )
        }
        None => OnScopeExitMut::new(prop_path, Box::new(|_| {})),
    }
}

pub fn validate_table_has_only_string_or_sequence_keys(
    table: &mlua::Table,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, _v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::String(_) => Ok(()),
            mlua::Value::Integer(i) => {
                if i >= 1 && i <= sequence_len {
                    Ok(())
                } else {
                    Err(mlua::Error::runtime(format!(
                        "In {}: Disjoint number indices not allowed: {}",
                        prop_path_string(prop_path.as_mut()),
                        i
                    )))
                }
            }
            _ => Err(mlua::Error::runtime(format!(
                "In {}: Expected string or integer index, but got a {}: {:?}",
                prop_path_string(prop_path.as_mut()),
                k.type_name(),
                k
            ))),
        }?;
    }
    Ok(())
}

pub fn validate_table_is_sequence(
    table: &mlua::Table,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, _v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::Integer(i) => {
                if i >= 1 && i <= sequence_len {
                    Ok(())
                } else {
                    Err(mlua::Error::runtime(format!(
                        "In {}: Sequence expected, but disjoint integer index found: {}",
                        prop_path_string(prop_path.as_mut()),
                        i
                    )))
                }
            }
            _ => Err(mlua::Error::runtime(format!(
                "In {}: Sequence expected, but non-integer index found: {:?}",
                prop_path_string(prop_path.as_mut()),
                k
            ))),
        }?;
    }

    Ok(())
}

pub fn validate_is_string<'a, 'lua>(
    value: &'a mlua::Value<'lua>,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<&'a mlua::String<'lua>> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);
    match value {
        mlua::Value::String(s) => Ok(s),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a string, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn validate_is_bool(
    value: &mlua::Value,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<bool> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    match value {
        mlua::Value::Boolean(b) => Ok(*b),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a boolean, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn validate_is_table<'a, 'lua>(
    value: &'a mlua::Value<'lua>,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<&'a mlua::Table<'lua>> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    match value {
        mlua::Value::Table(t) => Ok(t),
        _ => Err(mlua::Error::runtime(format!(
            "In {}: Expected a table, but got a {}: {:?}",
            prop_path_string(prop_path.as_mut()),
            value.type_name(),
            value
        ))),
    }
}

pub fn key_validation_error<T>(
    key: &str,
    valid_keys: Vec<&str>,
    prop_path: &Vec<Cow<'static, str>>,
) -> mlua::Result<T> {
    Err(mlua::Error::runtime(format!(
        "In {}: Unknonwn property name: '{}'. Expected one of [{}]",
        prop_path_string(prop_path),
        key,
        valid_keys.join(", ")
    )))
}

pub fn validate_required_key(
    table: &mlua::Table,
    key: &str,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    if table.contains_key(key)? {
        Ok(())
    } else {
        Err(mlua::Error::runtime(format!(
            "In {}: '{}' property is required",
            prop_path_string(prop_path.as_mut()),
            key
        )))
    }
}
