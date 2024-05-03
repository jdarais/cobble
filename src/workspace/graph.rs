use std::collections::{hash_map, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::datamodel::{Action, ActionCmd, Artifact, BuildEnv, Dependency, ExternalTool, Project, TaskDef};



#[derive(Clone, Debug)]
pub enum TaskType {
    Project,
    Task,
    BuildEnv
}

#[derive(Clone, Debug)]
pub struct FileDependency {
    pub path: String,
    pub provided_by_task: Option<String>
}

#[derive(Clone, Debug)]
pub struct Task {
    pub task_type: TaskType,
    pub project_name: String,
    pub dir: PathBuf,
    pub build_envs: HashMap<String, String>,
    pub tools: HashMap<String, String>,
    pub file_deps: Vec<FileDependency>,
    pub task_deps: Vec<String>,
    pub calc_deps: Vec<String>,
    pub actions: Vec<Action>,
    pub artifacts: Vec<Artifact>
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub tasks: HashMap<String, Arc<Task>>,
    pub build_envs: HashMap<String, Arc<BuildEnv>>,
    pub tools: HashMap<String, Arc<ExternalTool>>
}


pub fn add_dependency_to_task(dep: &Dependency, file_providers: &HashMap<&str, &str>, task: &mut Task) {
    match dep {
        Dependency::File(f) => {
            if !task.file_deps.iter().any(|dep| &dep.path == f) {
                task.file_deps.push(FileDependency {
                    path: f.clone(),
                    provided_by_task: file_providers.get(f.as_str()).map(|&t| t.to_owned())
                });
            }
        },
        Dependency::Task(t) => {
            if !task.task_deps.contains(t) {
                task.task_deps.push(t.clone());
            }
        },
        Dependency::Calc(c) => {
            if !task.calc_deps.contains(c) {
                task.calc_deps.push(c.clone());
            }
        }
    }
}

fn add_action_to_task(action: &Action, task: &mut Task) {
    for (env_alias, env_name) in action.build_envs.iter() {
        task.build_envs.insert(env_alias.clone(), env_name.clone());
    }

    for (tool_alias, tool_name) in action.tools.iter() {
        task.tools.insert(tool_alias.clone(), tool_name.clone());
    }

    match action.cmd {
        ActionCmd::Func(_) => {
            if let hash_map::Entry::Vacant(ent) = task.tools.entry(String::from("cmd")) {
                ent.insert(String::from("cmd"));
            }
        },
        ActionCmd::Cmd(_) => {
            if action.tools.len() == 0 {
                if let hash_map::Entry::Vacant(ent) = task.tools.entry(String::from("cmd")) {
                    ent.insert(String::from("cmd"));
                }
            }
        }
    }

    task.actions.push(action.clone());
}

fn add_build_env_to_workspace(build_env: &BuildEnv, project_name: &str, dir: &Path, file_providers: &HashMap<&str, &str>, workspace: &mut Workspace) {
    let mut install_task = Task {
        task_type: TaskType::BuildEnv,
        dir: PathBuf::from(dir),
        project_name: project_name.to_owned(),
        tools: HashMap::new(),
        build_envs: HashMap::new(),
        file_deps: Vec::new(),
        task_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: Vec::new()
    };

    for dep in build_env.deps.iter() {
        add_dependency_to_task(dep, file_providers, &mut install_task);
    }

    for install_action in build_env.install.iter() {
        add_action_to_task(install_action, &mut install_task);
    }

    workspace.tasks.insert(build_env.name.clone(), Arc::new(install_task));
    workspace.build_envs.insert(build_env.name.clone(), Arc::new(build_env.clone()));
}

fn add_task_to_workspace(task_def: &TaskDef, project_name: &str, dir: &Path, file_providers: &HashMap<&str, &str>, workspace: &mut Workspace) {
    let mut task = Task {
        task_type: TaskType::Task,
        dir: PathBuf::from(dir),
        project_name: project_name.to_owned(),
        tools: HashMap::new(),
        build_envs: task_def.build_env.iter().cloned().collect(),
        file_deps: Vec::new(),
        task_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: task_def.artifacts.iter().cloned().collect()
    };

    for dep in task_def.deps.iter() {
        add_dependency_to_task(dep, file_providers, &mut task);
    }

    for action in task_def.actions.iter() {
        add_action_to_task(action, &mut task);
    }

    workspace.tasks.insert(task_def.name.clone(), Arc::new(task));
}

fn add_project_to_workspace(project: &Project, file_providers: &HashMap<&str, &str>, workspace: &mut Workspace) {
    if project.name != "/__COBBLE_INTERNAL__" {
        workspace.tasks.insert(project.name.clone(), Arc::new(
        Task {
                task_type: TaskType::Project,
                dir: project.path.clone(),
                project_name: project.name.clone(),
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                file_deps: Vec::new(),
                task_deps: project.tasks.iter().map(|t| t.name.clone())
                    .chain(project.child_project_names.iter().map(|name| name.clone()))
                    .collect(),
                calc_deps: Vec::new(),
                actions: Vec::new(),
                artifacts: Vec::new()
            }
        ));
    }

    for env in project.build_envs.iter() {
        add_build_env_to_workspace(env, project.name.as_str(), project.path.as_path(), file_providers, workspace);
    }

    for task in project.tasks.iter() {
        add_task_to_workspace(task, project.name.as_str(), project.path.as_path(), file_providers, workspace);
    }

    for tool in project.tools.iter() {
        workspace.tools.insert(tool.name.clone(), Arc::new(tool.clone()));
    }
}



pub fn create_workspace<'a, P>(all_projects: P, file_providers: &HashMap<&'a str, &'a str>) -> Workspace
    where P: Iterator<Item = &'a Project>
{
    let mut workspace = Workspace {
        tasks: HashMap::new(),
        build_envs: HashMap::new(),
        tools: HashMap::new()
    };

    let projects_vec: Vec<&'a Project> = all_projects.collect();

    for project in projects_vec.iter().copied() {
        add_project_to_workspace(project, &file_providers, &mut workspace);
    }

    workspace
}
