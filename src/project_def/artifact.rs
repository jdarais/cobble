use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use crate::project_def::validate::validate_is_string;

use super::validate::{key_validation_error, push_prop_name_if_exists, validate_is_table, validate_table_has_only_string_or_sequence_keys, validate_table_is_sequence};

#[derive(Clone, Debug, Default)]
pub struct Artifacts {
    pub files: Vec<Arc<str>>,
    pub calc: Vec<Arc<str>>
}

pub fn validate_artifact<'lua>(
    _lua: &'lua mlua::Lua,
    value: &mlua::Value<'lua>,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    validate_is_string(value, prop_name, prop_path).and(Ok(()))
}

impl fmt::Display for Artifacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Artifact(files=[")?;
        for (i, filename) in self.files.iter().enumerate() {
            if i > 0 { f.write_str(",")?; }
            f.write_str(filename.as_ref())?;
        }
        f.write_str("], calc=[")?;
        for (i, calc) in self.calc.iter().enumerate() {
            if i > 0 { f.write_str(",")?; }
            f.write_str(calc.as_ref())?;
        }
        f.write_str("])")?;

        Ok(())
    }
}

pub fn validate_artifacts<'lua>(
    value: &mlua::Value<'lua>,
    prop_name: Option<Cow<'static, str>>,
    prop_path: &mut Vec<Cow<'static, str>>,
) -> mlua::Result<()> {
    let mut prop_path = push_prop_name_if_exists(prop_name, prop_path);

    let table_value = validate_is_table(value, None, prop_path.as_mut())?;
    validate_table_has_only_string_or_sequence_keys(&table_value, None, prop_path.as_mut())?;

    for pair in table_value.clone().pairs() {
        let (k, v): (mlua::Value, mlua::Value) = pair?;
        
        if let mlua::Value::String(k_string) = k {
            let k_str = k_string.to_str()?;
            match k_str {
                "files" => {
                    let files_table = validate_is_table(&v, Some(Cow::Borrowed("files")), prop_path.as_mut())?;
                    validate_table_is_sequence(files_table, Some(Cow::Borrowed("files")), prop_path.as_mut())?;
                    for f_val in files_table.clone().sequence_values() {
                        let f: mlua::Value = f_val?;
                        validate_is_string(&f, None, prop_path.as_mut())?;
                    }
                },
                "calc" => {
                    let calc_table = validate_is_table(&v, Some(Cow::Borrowed("files")), prop_path.as_mut())?;
                    validate_table_is_sequence(calc_table, Some(Cow::Borrowed("files")), prop_path.as_mut())?;
                    for c_val in calc_table.clone().sequence_values() {
                        let c: mlua::Value = c_val?;
                        validate_is_string(&c, None, prop_path.as_mut())?;
                    }
                },
                _ => key_validation_error(k_str, vec!["files", "calc"], prop_path.as_mut())?
            }
        }
    }

    for f_val in table_value.clone().sequence_values() {
        let f: mlua::Value = f_val?;
        validate_is_string(&f, None, prop_path.as_mut())?;
    }

    Ok(())
}

impl<'lua> mlua::FromLua<'lua> for Artifacts {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::Table(table_value) => {
                let mut files: Vec<Arc<str>> = Vec::new();
                let mut calc: Vec<Arc<str>> = Vec::new();

                let files_value_opt: Option<Vec<String>> = table_value.get("files")?;
                if let Some(files_value) = files_value_opt {
                    for f in files_value {
                        files.push(Arc::<str>::from(f));
                    }
                }

                let calc_value_opt: Option<Vec<String>> = table_value.get("calc")?;
                if let Some(calc_value) = calc_value_opt {
                    for c in calc_value {
                        calc.push(Arc::<str>::from(c));
                    }
                }

                // Treat sequence values as files
                for val in table_value.sequence_values() {
                    let f: String = val?;
                    files.push(Arc::<str>::from(f));
                }

                Ok(Artifacts { files, calc })
            },
            _ => Err(mlua::Error::RuntimeError(format!("Expected a table, but got a {}", value.type_name())))
        }
    }
}
