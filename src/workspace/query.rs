use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};

use crate::datamodel::{Action, Artifact, BuildEnv, Dependency, ExternalTool, Project, Task};

#[derive(Clone, Debug)]
pub enum WorkspaceTargetType {
    Project,
    Task,
    BuildEnv
}

#[derive(Clone, Debug)]
pub struct WorkspaceTarget {
    pub target_type: WorkspaceTargetType,
    pub dir: PathBuf,
    pub build_envs: HashMap<String, String>,
    pub tools: HashMap<String, String>,
    pub file_deps: Vec<String>,
    pub target_deps: Vec<String>,
    pub calc_deps: Vec<String>,
    pub actions: Vec<Action>,
    pub artifacts: Vec<Artifact>
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub targets: HashMap<String, Arc<WorkspaceTarget>>,
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

pub fn find_targets_for_dir<'a>(workspace: &'a Workspace, workspace_dir: &Path, project_dir: &Path) -> Vec<&'a str> {
    let full_project_dir = workspace_dir.join(project_dir);
    workspace.targets.iter()
        .filter(|(_k, v)| workspace_dir.join(v.dir.as_path()).starts_with(&full_project_dir))
        .map(|(k, _v)| k.as_str())
        .collect()
}

fn add_dependency_to_target(dep: &Dependency, target: &mut WorkspaceTarget) {
    match dep {
        Dependency::File(f) => {
            if !target.file_deps.contains(f) {
                target.file_deps.push(f.clone());
            }
        },
        Dependency::Task(t) => {
            if !target.target_deps.contains(t) {
                target.target_deps.push(t.clone());
            }
        },
        Dependency::Calc(c) => {
            if !target.calc_deps.contains(c) {
                target.calc_deps.push(c.clone());
            }
        }
    }
}

fn add_action_to_target(action: &Action, target: &mut WorkspaceTarget) {
    for (env_alias, env_name) in action.build_envs.iter() {
        target.build_envs.insert(env_alias.clone(), env_name.clone());
    }

    for (tool_alias, tool_name) in action.tools.iter() {
        target.tools.insert(tool_alias.clone(), tool_name.clone());
    }

    target.actions.push(action.clone());
}

fn add_build_env_to_workspace(build_env: &BuildEnv, dir: &Path, workspace: &mut Workspace) {
    let mut install_target = WorkspaceTarget {
        target_type: WorkspaceTargetType::BuildEnv,
        dir: PathBuf::from(dir),
        tools: HashMap::new(),
        build_envs: HashMap::new(),
        file_deps: Vec::new(),
        target_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: Vec::new()
    };

    for dep in build_env.deps.iter() {
        add_dependency_to_target(dep, &mut install_target);
    }

    for install_action in build_env.install.iter() {
        add_action_to_target(install_action, &mut install_target);
    }

    workspace.targets.insert(build_env.name.clone(), Arc::new(install_target));
    workspace.build_envs.insert(build_env.name.clone(), Arc::new(build_env.clone()));
}

fn add_task_to_workspace(task: &Task, dir: &Path, workspace: &mut Workspace) {
    let mut task_target = WorkspaceTarget {
        target_type: WorkspaceTargetType::Task,
        dir: PathBuf::from(dir),
        tools: HashMap::new(),
        build_envs: task.build_env.iter().cloned().collect(),
        file_deps: Vec::new(),
        target_deps: Vec::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: task.artifacts.iter().cloned().collect()
    };

    for dep in task.deps.iter() {
        add_dependency_to_target(dep, &mut task_target);
    }

    for action in task.actions.iter() {
        add_action_to_target(action, &mut task_target);
    }

    workspace.targets.insert(task.name.clone(), Arc::new(task_target));
}

fn add_project_to_workspace(project: &Project, workspace: &mut Workspace) {
    workspace.targets.insert(project.name.clone(), Arc::new(
    WorkspaceTarget {
            target_type: WorkspaceTargetType::Project,
            dir: project.path.clone(),
            tools: HashMap::new(),
            build_envs: HashMap::new(),
            file_deps: Vec::new(),
            target_deps: project.tasks.iter().map(|t| t.name.clone())
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

pub fn get_all_targets<'a, P>(all_projects: P) -> Workspace
    where P: Iterator<Item = &'a Project>
{
    let mut workspace = Workspace {
        targets: HashMap::new(),
        build_envs: HashMap::new(),
        tools: HashMap::new()
    };
    for project in all_projects {
        add_project_to_workspace(project, &mut workspace);
    }
    workspace
}

