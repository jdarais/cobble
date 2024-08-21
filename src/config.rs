// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::project_def::types::TaskVar;
use crate::vars::{set_var, VarLookupError};

pub const WORKSPACE_CONFIG_FILE_NAME: &str = "cobble.toml";
pub const PROJECT_FILE_NAME: &str = "project.lua";

pub const DEFAULT_NUM_THREADS: u8 = 5;

#[derive(Debug)]
pub struct WorkspaceConfig {
    pub workspace_dir: PathBuf,
    pub root_projects: Vec<String>,
    pub vars: HashMap<String, TaskVar>,
    pub force_run_tasks: bool,
    pub num_threads: u8,
    pub show_stdout: TaskOutputCondition,
    pub show_stderr: TaskOutputCondition
}

#[derive(Default)]
pub struct WorkspaceConfigArgs {
    pub vars: Vec<String>,
    pub force_run_tasks: Option<bool>,
    pub num_threads: Option<u8>,
    pub show_stdout: Option<TaskOutputCondition>,
    pub show_stderr: Option<TaskOutputCondition>
}

#[derive(Debug)]
pub enum WorkspaceConfigError {
    IOError(io::Error),
    FileError { path: PathBuf, error: io::Error },
    ParseError(String),
    ValueError(String),
    SetVarError(VarLookupError),
}

impl Error for WorkspaceConfigError {}
impl Display for WorkspaceConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use WorkspaceConfigError::*;
        match self {
            IOError(e) => write!(f, "{}", e),
            FileError { path, error } => {
                write!(f, "Error reading file at {}: {}", path.display(), error)
            }
            ParseError(msg) => write!(f, "Error parsing config file: {}", msg),
            ValueError(msg) => write!(f, "Error reading config values: {}", msg),
            SetVarError(e) => write!(f, "Error setting variable: {}", e),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TaskOutputCondition {
    Always,
    Never,
    OnFail,
}

impl <'lua> mlua::FromLua<'lua> for TaskOutputCondition {
    fn from_lua(value: mlua::Value<'lua>, _lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        match value {
            mlua::Value::String(s) => {
                parse_output_condition(s.to_str()?).map_err(|e| mlua::Error::runtime(e))
            },
            invalid_value => Err(mlua::Error::runtime(format!("Expected a string value for output condition, but got a  {}.", invalid_value.type_name())))
        }
    }
}

pub fn parse_output_condition(value: &str) -> Result<TaskOutputCondition, String> {
    let value_lower = value.to_lowercase();
    match value_lower.as_str() {
        "always" => Ok(TaskOutputCondition::Always),
        "never" => Ok(TaskOutputCondition::Never),
        "on_fail" => Ok(TaskOutputCondition::OnFail),
        invalid_val => Err(format!("Invalid value given for output condition: {}.  Expected one of [always, never, on_fail].", invalid_val))
    }
}

pub fn parse_workspace_config(
    config_str: &str,
    config_path: &Path,
) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config: toml::Table = config_str
        .parse()
        .map_err(|e| WorkspaceConfigError::ParseError(format!("Error parsing config: {}", e)))?;

    // Root Projects
    let root_projects_opt: Option<toml::Value> = config.remove("root_projects");
    let root_projects: Vec<String> = match root_projects_opt {
        None => vec![String::from(".")],
        Some(val) => val
            .try_into()
            .map_err(|e| WorkspaceConfigError::ValueError(format!("at 'root_projects': {}", e)))?,
    };

    // Num Threads
    let num_threads_opt: Option<toml::Value> = config.remove("num_threads");
    let num_threads: u8 = match num_threads_opt {
        Some(val) => val.try_into().map_err(|e| WorkspaceConfigError::ValueError(format!("at 'num_threads': {}", e)))?,
        None => DEFAULT_NUM_THREADS
    };

    // Task Output
    let output_opt: Option<toml::Value> = config.remove("output");
    let output = match output_opt {
        Some(output_val) => {
            let output_str: String = output_val.try_into().map_err(|e| WorkspaceConfigError::ValueError(format!("at 'output': {}", e)))?;
            let output_enum = parse_output_condition(output_str.as_str()).map_err(|e| WorkspaceConfigError::ValueError(format!("at 'output': {}", e)))?;
            output_enum
        }
        None => TaskOutputCondition::OnFail
    };

    let stdout_opt: Option<toml::Value> = config.remove("stdout");
    let stdout = match stdout_opt {
        Some(stdout_val) => {
            let stdout_str: String = stdout_val.try_into().map_err(|e| WorkspaceConfigError::ValueError(format!("at 'stdout': {}", e)))?;
            let stdout_enum = parse_output_condition(stdout_str.as_str()).map_err(|e| WorkspaceConfigError::ValueError(format!("at 'stdout': {}", e)))?;
            stdout_enum
        }
        None => output.clone()
    };

    let stderr_opt: Option<toml::Value> = config.remove("stderr");
    let stderr = match stderr_opt {
        Some(stderr_val) => {
            let stderr_str: String = stderr_val.try_into().map_err(|e| WorkspaceConfigError::ValueError(format!("at 'stderr': {}", e)))?;
            let stderr_enum = parse_output_condition(stderr_str.as_str()).map_err(|e| WorkspaceConfigError::ValueError(format!("at 'stderr': {}", e)))?;
            stderr_enum
        }
        None => output
    };

    // Vars
    let mut vars: HashMap<String, TaskVar> = HashMap::new();
    let vars_val: toml::Value = config
        .remove("vars")
        .unwrap_or_else(|| toml::Value::Table(toml::Table::new()));
    let vars_table = match vars_val {
        toml::Value::Table(t) => t,
        _ => {
            return Err(WorkspaceConfigError::ValueError(String::from(
                "vars config variable must be a table",
            )));
        }
    };
    for (k, v) in vars_table {
        vars.insert(k, v.into());
    }

    // Raise an error if there are unrecognized keys in the config table
    if let Some((key, _)) = config.iter().next() {
        return Err(WorkspaceConfigError::ValueError(format!(
            "Unrecognized field '{}'",
            key
        )));
    }

    Ok(WorkspaceConfig {
        workspace_dir: PathBuf::from(config_path.parent().unwrap_or_else(|| Path::new("."))),
        root_projects,
        vars,
        force_run_tasks: false,
        num_threads,
        show_stdout: stdout,
        show_stderr: stderr
    })
}

pub fn parse_workspace_config_file(path: &Path) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let mut config_file = File::open(path).map_err(|e| WorkspaceConfigError::FileError {
        path: PathBuf::from(path),
        error: e,
    })?;

    let mut config_toml_str = String::new();
    let file_read_res = config_file.read_to_string(&mut config_toml_str);
    if let Err(e) = file_read_res {
        return Err(WorkspaceConfigError::FileError {
            path: PathBuf::from(path),
            error: e,
        });
    }

    parse_workspace_config(config_toml_str.as_str(), path)
}

pub fn find_nearest_workspace_config_file_from(path: &Path) -> Result<PathBuf, io::Error> {
    for ancestor in dunce::canonicalize(path)?.ancestors() {
        let config_path = ancestor.join(WORKSPACE_CONFIG_FILE_NAME);
        if config_path.exists() {
            return Ok(config_path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "Did not find '{}' file in any ancestor directory from {}",
            WORKSPACE_CONFIG_FILE_NAME,
            path.display()
        ),
    ))
}

/// Returns the closest project directory to the given path in the workspace at workspace_dir.
/// The returned path is relative to the workspace directory
pub fn find_nearest_project_dir(path: &Path, workspace_dir: &Path) -> Result<PathBuf, io::Error> {
    let canonicalized_workspace_dir = workspace_dir.canonicalize()?;
    for ancestor in path.canonicalize()?.ancestors() {
        if !ancestor.starts_with(canonicalized_workspace_dir.as_path()) {
            break;
        }

        let project_file_path = ancestor.join(PROJECT_FILE_NAME);
        if project_file_path.exists() {
            let project_path = PathBuf::from(ancestor);
            let rel_project_path = project_path
                .strip_prefix(canonicalized_workspace_dir.as_path())
                .expect("project path starts with workspace path");
            return Ok(PathBuf::from_iter(
                Path::new(".").join(rel_project_path).components(),
            ));
        }
    }

    Ok(PathBuf::from("."))
}

pub fn get_workspace_config(
    path: &Path,
    args: &WorkspaceConfigArgs,
) -> Result<WorkspaceConfig, WorkspaceConfigError> {
    let config_path = find_nearest_workspace_config_file_from(path)
        .map_err(|e| WorkspaceConfigError::IOError(e))?;
    let mut config = parse_workspace_config_file(config_path.as_path())?;

    if let Some(force_run_tasks) = args.force_run_tasks {
        config.force_run_tasks = force_run_tasks;
    }

    if let Some(num_threads) = args.num_threads {
        config.num_threads = num_threads;
    }

    if let Some(show_stdout) = &args.show_stdout {
        config.show_stdout = show_stdout.clone();
    }

    if let Some(show_stderr) = &args.show_stderr {
        config.show_stderr = show_stderr.clone();
    }

    add_cli_vars_to_workspace_config(args.vars.iter().map(String::as_str), &mut config)?;

    Ok(config)
}

fn add_cli_vars_to_workspace_config<'a, I>(
    vars: I,
    config: &mut WorkspaceConfig,
) -> Result<(), WorkspaceConfigError>
where
    I: Iterator<Item = &'a str>,
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
        let var_val = &var[eq_idx + 1..];

        set_var(
            var_name,
            TaskVar::String(var_val.to_owned()),
            &mut config.vars,
        )
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

        let config =
            parse_workspace_config(config_toml, Path::new("/home/test/proj/cobble.toml")).unwrap();
        assert_eq!(config.workspace_dir, PathBuf::from("/home/test/proj"));
        assert_eq!(config.root_projects, vec!["proj1", "proj2", "proj3"]);
    }
}
