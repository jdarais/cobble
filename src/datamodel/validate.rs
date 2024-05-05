pub fn validate_table_has_only_string_or_sequence_keys(table: &mlua::Table) -> mlua::Result<()> {
    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::String(_) => Ok(()),
            mlua::Value::Integer(i) => if i >= 1 && i <= sequence_len {
                Ok(())
            } else  {
                Err(mlua::Error::runtime(format!("Disjoint number indices not allowed: {}", i)))
            },
            _ => Err(mlua::Error::runtime(format!("Expected string or integer index, but got a {}: {:?}", k.type_name(), k)))
        }?;
        match v {
            mlua::Value::String(_) => Ok(()),
            _ => Err(mlua::Error::runtime(format!("Expected a string, but got a {}: {:?}", v.type_name(), v)))
        }?;
    }
    Ok(())
}

pub fn validate_table_is_sequence(table: &mlua::Table) -> mlua::Result<()> {
    let sequence_len = table.len()?;
    for pair in table.clone().pairs() {
        let (k, _v): (mlua::Value, mlua::Value) = pair?;
        match k {
            mlua::Value::Integer(i) => if i >= 1 && i <= sequence_len { Ok(()) }
                else { Err(mlua::Error::runtime(format!("Sequence expected, but disjoint integer index found: {}", i))) },
            _ => Err(mlua::Error::runtime(format!("Sequence expected, but non-integer index found: {:?}", k)))
        }?;
    }

    Ok(())
}

pub fn validate_is_string<'a, 'lua>(value: &'a mlua::Value<'lua>) -> mlua::Result<&'a mlua::String<'lua>> {
    match value {
        mlua::Value::String(s) => Ok(s),
        _ => Err(mlua::Error::runtime(format!("Expected a string, but got a {}: {:?}", value.type_name(), value)))
    }
}

pub fn validate_is_bool(value: &mlua::Value) -> mlua::Result<bool> {
    match value {
        mlua::Value::Boolean(b) => Ok(*b),
        _ => Err(mlua::Error::runtime(format!("Expected a boolean, but got a {}: {:?}", value.type_name(), value)))
    }
}

pub fn validate_is_table<'a, 'lua>(value: &'a mlua::Value<'lua>) -> mlua::Result<&'a mlua::Table<'lua>> {
    match value {
        mlua::Value::Table(t) => Ok(t),
        _ => Err(mlua::Error::runtime(format!("Expected a table, but got a {}: {:?}", value.type_name(), value)))
    }
}

pub fn key_validation_error<T>(key: &str, valid_keys: Vec<&str>) -> mlua::Result<T> {
    Err(mlua::Error::runtime(format!("Unknonwn property name: '{}'. Expected one of [{}]", key, valid_keys.join(", "))))
}

pub fn validate_required_key(table: &mlua::Table, key: &str) -> mlua::Result<()> {
    if table.contains_key(key)? {
        Ok(())
    } else {
        Err(mlua::Error::runtime("'name' property is required"))
    }
}

