use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::datamodel::types::TaskVar;
use crate::workspace::vars::{set_var, VarLookupError};

pub const WORKSPACE_CONFIG_FILE_NAME: &str = "cobble.toml";
pub const PROJECT_FILE_NAME: &str = "project.lua";

#[derive(Debug)]
pub struct WorkspaceConfig {
    pub workspace_dir: PathBuf,
    pub root_projects: Vec<String>,
    pub vars: HashMap<String, TaskVar>
}

#[derive(Debug)]
pub enum WorkspaceConfigError {
    IOError(io::Error),
    FileError{path: PathBuf, error: io::Error},
    ParseError(String),
    ValueError(String),
    SetVarError(VarLookupError)
}

impl Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use WorkspaceConfigError::*;
        match self {
            IOError(e) => write!(f, "{}", e),
            FileError{path, error} => write!(f, "Error reading file at {}: {}", path.display(), error),
            ParseError(msg) => write!(f, "Error parsing config file: {}", msg),
            ValueError(msg) => write!(f, "Error reading config values: {}", msg),
            SetVarError(e) => write!(f, "Error setting variable: {}", e)
        }
    }
}

pub fn parse_workspace_config(config_str: &str, config_path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config: toml::Table = config_str.parse().map_err(|e| WorkspaceConfigError::ParseError(format!("Error parsing config: {}", e)))?;

    let root_projects_opt: Option<toml::Value> = config.remove("root_projects");
    let root_projects: Vec<String> = match root_projects_opt {
        None => vec![String::from(".")],
        Some(val) => val.try_into()
            .map_err(|e| WorkspaceConfigError::ValueError(format!("at 'root_projects': {}", e)))?
    };

    let mut vars: HashMap<String, TaskVar> = HashMap::new();
    let vars_val: toml::Value = config.remove("vars").unwrap_or_else(|| toml::Value::Table(toml::Table::new()));
    let vars_table = match vars_val {
        toml::Value::Table(t) => t,
        _ => { return Err(WorkspaceConfigError::ValueError(String::from("vars config variable must be a table"))); }
    };
    for (k, v) in vars_table {
        vars.insert(k, v.into());
    }

    // Raise an error if there are unrecognized keys in the config table
    if let Some((key, _)) = config.iter().next() {
        return Err(WorkspaceConfigError::ValueError(format!("Unrecognized field '{}'", key)));
    }

    Ok(WorkspaceConfig{
        workspace_dir: PathBuf::from(config_path.parent().unwrap_or_else(|| Path::new("."))),
        root_projects,
        vars
    })
}

pub fn parse_workspace_config_file(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config_file = File::open(path).map_err(|e| WorkspaceConfigError::FileError{path: PathBuf::from(path), error: e})?;

    let mut config_toml_str = String::new();
    let file_read_res = config_file.read_to_string(&mut config_toml_str);
    if let Err(e) = file_read_res {
        return Err(WorkspaceConfigError::FileError{path: PathBuf::from(path), error: e});
    }

    parse_workspace_config(config_toml_str.as_str(), path)
}

pub fn find_nearest_workspace_config_file_from(path: &Path) -> Result<PathBuf, io::Error> {
    for ancestor in path.canonicalize()?.ancestors() {
        let config_path = ancestor.join(WORKSPACE_CONFIG_FILE_NAME);
        if config_path.exists() {
            return Ok(config_path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Did not find '{}' file in any ancestor directory from {}", WORKSPACE_CONFIG_FILE_NAME, path.display()))
    )
}

/// Returns the closest project directory to the given path in the workspace at workspace_dir.
/// The returned path is relative to the workspace directory
pub fn find_nearest_project_dir(path: &Path, workspace_dir: &Path) -> Result<PathBuf, io::Error> {
    for ancestor in path.canonicalize()?.ancestors() {
        if !ancestor.starts_with(workspace_dir) {
            break;
        }

        let project_file_path = ancestor.join(PROJECT_FILE_NAME);
        if project_file_path.exists() {
            let project_path = PathBuf::from(ancestor);
            let rel_project_path = project_path.strip_prefix(workspace_dir).expect("project path starts with workspace path");
            return Ok(PathBuf::from_iter(Path::new(".").join(rel_project_path).components()))
        }
    }

    Ok(PathBuf::from("."))
}

pub fn get_workspace_config(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let config_path = find_nearest_workspace_config_file_from(path).map_err(|e| WorkspaceConfigError::IOError(e))?;
    parse_workspace_config_file(config_path.as_path())
}

pub fn add_cli_vars_to_workspace_config<'a, I>(vars: I, config: &mut WorkspaceConfig) -> Result<(), WorkspaceConfigError>
where I: Iterator<Item = &'a str>
{
    for var in vars {
        let eq_idx = match var.find("=") {
            Some(i) => i,
            None => {
                return Err(WorkspaceConfigError::ValueError(
                    format!("Unable to parse variable argument '{}'.  Specify variable arguments in the form '--var <the.var.name>=<value>'.", var)
                ));
            }
        };
    
        let var_name = &var[..eq_idx];
        let var_val = &var[eq_idx+1..];

        set_var(var_name, TaskVar::String(var_val.to_owned()), &mut config.vars)
            .map_err(|e| WorkspaceConfigError::SetVarError(e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_parse_workspace_config() {
        let config_toml = r#"
            root_projects = ["proj1", "proj2", "proj3"]
        "#;

        let config = parse_workspace_config(config_toml, Path::new("/home/test/proj/cobble.toml")).unwrap();
        assert_eq!(config.workspace_dir, PathBuf::from("/home/test/proj"));
        assert_eq!(config.root_projects, vec!["proj1", "proj2", "proj3"]);
    }
}