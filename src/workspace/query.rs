use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::datamodel::{Action, Artifact, BuildEnv, Dependency, ExternalTool, Project, TaskDef};

#[derive(Clone, Debug)]
pub enum TaskType {
    Project,
    Task,
    BuildEnv
}

#[derive(Clone, Debug)]
pub struct Task {
    pub task_type: TaskType,
    pub dir: PathBuf,
    pub build_envs: HashMap<String, String>,
    pub tools: HashMap<String, String>,
    pub file_deps: Vec<String>,
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

pub fn find_project_for_dir<'a, P>(all_projects: P, workspace_dir: &Path, project_dir: &Path) -> Option<&'a Project>
    where P: Iterator<Item = &'a Project>
{
    for project in all_projects {
        println!("Comparing {} and {}", project.path.display(), project_dir.display());
        if workspace_dir.join(project.path.as_path()) == workspace_dir.join(project_dir) {
            return Some(project)
        }
    }

    None
}

pub fn find_tasks_for_dir<'a>(workspace: &'a Workspace, workspace_dir: &Path, project_dir: &Path) -> Vec<&'a str> {
    let full_project_dir = workspace_dir.join(project_dir);
    workspace.tasks.iter()
        .filter(|(_k, v)| workspace_dir.join(v.dir.as_path()).starts_with(&full_project_dir))
        .map(|(k, _v)| k.as_str())
        .collect()
}

fn add_dependency_to_task(dep: &Dependency, task: &mut Task) {
    match dep {
        Dependency::File(f) => {
            if !task.file_deps.contains(f) {
                task.file_deps.push(f.clone());
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

    task.actions.push(action.clone());
}

fn add_build_env_to_workspace(build_env: &BuildEnv, dir: &Path, workspace: &mut Workspace) {
    let mut install_task = Task {
        task_type: TaskType::BuildEnv,
        dir: PathBuf::from(dir),
        tools: HashMap::new(),
        build_envs: HashMap::new(),
        file_deps: Vec::new(),
        task_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: Vec::new()
    };

    for dep in build_env.deps.iter() {
        add_dependency_to_task(dep, &mut install_task);
    }

    for install_action in build_env.install.iter() {
        add_action_to_task(install_action, &mut install_task);
    }

    workspace.tasks.insert(build_env.name.clone(), Arc::new(install_task));
    workspace.build_envs.insert(build_env.name.clone(), Arc::new(build_env.clone()));
}

fn add_task_to_workspace(task_def: &TaskDef, dir: &Path, workspace: &mut Workspace) {
    let mut task = Task {
        task_type: TaskType::Task,
        dir: PathBuf::from(dir),
        tools: HashMap::new(),
        build_envs: task_def.build_env.iter().cloned().collect(),
        file_deps: Vec::new(),
        task_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: task_def.artifacts.iter().cloned().collect()
    };

    for dep in task_def.deps.iter() {
        add_dependency_to_task(dep, &mut task);
    }

    for action in task_def.actions.iter() {
        add_action_to_task(action, &mut task);
    }

    workspace.tasks.insert(task_def.name.clone(), Arc::new(task));
}

fn add_project_to_workspace(project: &Project, workspace: &mut Workspace) {
    workspace.tasks.insert(project.name.clone(), Arc::new(
    Task {
            task_type: TaskType::Project,
            dir: project.path.clone(),
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

    for env in project.build_envs.iter() {
        add_build_env_to_workspace(env, project.path.as_path(), workspace);
    }

    for task in project.tasks.iter() {
        add_task_to_workspace(task, project.path.as_path(), workspace);
    }

    for tool in project.tools.iter() {
        workspace.tools.insert(tool.name.clone(), Arc::new(tool.clone()));
    }
}

pub fn get_all_tasks<'a, P>(all_projects: P) -> Workspace
    where P: Iterator<Item = &'a Project>
{
    let mut workspace = Workspace {
        tasks: HashMap::new(),
        build_envs: HashMap::new(),
        tools: HashMap::new()
    };
    for project in all_projects {
        add_project_to_workspace(project, &mut workspace);
    }
    workspace
}

