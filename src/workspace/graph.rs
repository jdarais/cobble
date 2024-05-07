use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::datamodel::{dependency::Dependencies, Action, Artifact, BuildEnv, ExternalTool, Project, TaskDef};



#[derive(Clone, Debug)]
pub enum TaskType {
    Project,
    Task,
    BuildEnv
}

#[derive(Clone, Debug)]
pub struct FileDependency {
    pub path: Arc<str>,
    pub provided_by_task: Option<Arc<str>>
}

#[derive(Clone, Debug)]
pub struct Task {
    pub task_type: TaskType,
    pub project_name: Arc<str>,
    pub dir: Arc<Path>,
    pub build_envs: HashMap<Arc<str>, Arc<str>>,
    pub tools: HashMap<Arc<str>, Arc<str>>,
    pub file_deps: HashMap<Arc<str>, FileDependency>,
    pub task_deps: HashMap<Arc<str>, Arc<str>>,
    pub var_deps: HashMap<Arc<str>, Arc<str>>,
    pub calc_deps: Vec<Arc<str>>,
    pub actions: Vec<Action>,
    pub artifacts: Vec<Artifact>
}

#[derive(Clone, Debug)]
pub struct Workspace {
    pub tasks: HashMap<Arc<str>, Arc<Task>>,
    pub build_envs: HashMap<Arc<str>, Arc<BuildEnv>>,
    pub tools: HashMap<Arc<str>, Arc<ExternalTool>>
}


pub fn add_dependency_list_to_task(deps: &Dependencies, file_providers: &HashMap<Arc<str>, Arc<str>>, task: &mut Task) {
    for (f_alias, f_path) in deps.files.iter() {
        task.file_deps.insert(f_alias.clone(), FileDependency {
            path: f_path.clone(),
            provided_by_task: file_providers.get(f_path).cloned()
        });
    }

    for (t_alias, t_path) in deps.tasks.iter() {
        task.task_deps.insert(t_alias.clone(), t_path.clone());
    }

    for (v_alias, v_path) in deps.vars.iter() {
        task.var_deps.insert(v_alias.clone(), v_path.clone());
    }
    
    for c_dep in deps.calc.iter() {
        if !task.calc_deps.contains(c_dep) {
            task.calc_deps.push(c_dep.clone());
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

fn add_build_env_to_workspace(build_env: &BuildEnv, project_name: &Arc<str>, dir: &Arc<Path>, file_providers: &HashMap<Arc<str>, Arc<str>>, workspace: &mut Workspace) {
    let mut install_task = Task {
        task_type: TaskType::BuildEnv,
        dir: dir.clone(),
        project_name: project_name.clone(),
        tools: HashMap::new(),
        build_envs: HashMap::new(),
        file_deps: HashMap::new(),
        task_deps: HashMap::new(),
        var_deps: HashMap::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: Vec::new()
    };

    add_dependency_list_to_task(&build_env.deps, file_providers, &mut install_task);

    for install_action in build_env.install.iter() {
        add_action_to_task(install_action, &mut install_task);
    }

    workspace.tasks.insert(build_env.name.clone(), Arc::new(install_task));
    workspace.build_envs.insert(build_env.name.clone(), Arc::new(build_env.clone()));
}

fn add_task_to_workspace(task_def: &TaskDef, project_name: &Arc<str>, dir: &Arc<Path>, file_providers: &HashMap<Arc<str>, Arc<str>>, workspace: &mut Workspace) {
    let mut task = Task {
        task_type: TaskType::Task,
        dir: dir.clone(),
        project_name: project_name.clone(),
        tools: HashMap::new(),
        build_envs: task_def.build_env.iter().cloned().collect(),
        file_deps: HashMap::new(),
        task_deps: HashMap::new(),
        var_deps: HashMap::new(),
        calc_deps: Vec::new(),
        actions: Vec::new(),
        artifacts: task_def.artifacts.iter().cloned().collect()
    };

    add_dependency_list_to_task(&task_def.deps, file_providers, &mut task);

    for action in task_def.actions.iter() {
        add_action_to_task(action, &mut task);
    }

    workspace.tasks.insert(task_def.name.clone(), Arc::new(task));
}

fn add_project_to_workspace(project: &Project, file_providers: &HashMap<Arc<str>, Arc<str>>, workspace: &mut Workspace) {
    if project.name.as_ref() != "/__COBBLE_INTERNAL__" {
        let mut project_task = Task {
                task_type: TaskType::Project,
                dir: project.path.clone(),
                project_name: project.name.clone(),
                tools: HashMap::new(),
                build_envs: HashMap::new(),
                file_deps: HashMap::new(),
                task_deps: project.child_project_names.iter().map(|t| (t.clone(), t.clone())).collect(),
                var_deps: HashMap::new(),
                calc_deps: Vec::new(),
                actions: Vec::new(),
                artifacts: Vec::new()
            };
        let mut default_tasks: Vec<&TaskDef> = project.tasks.iter().filter(|t| t.is_default.unwrap_or(false)).collect();
        if default_tasks.len() == 0 {
            // Not specifying any default tasks for a project results in all tasks being default
            default_tasks = project.tasks.iter().collect();
        }

        for task in default_tasks.into_iter() {
            project_task.task_deps.insert(task.name.clone(), task.name.clone());
        }

        workspace.tasks.insert(project.name.clone(), Arc::new(project_task));
    }

    for env in project.build_envs.iter() {
        add_build_env_to_workspace(env, &project.name, &project.path, file_providers, workspace);
    }

    for task in project.tasks.iter() {
        add_task_to_workspace(task, &project.name, &project.path, file_providers, workspace);
    }

    for tool in project.tools.iter() {
        workspace.tools.insert(tool.name.clone(), Arc::new(tool.clone()));
    }
}



pub fn create_workspace<'a, P>(all_projects: P, file_providers: &HashMap<Arc<str>, Arc<str>>) -> Workspace
    where P: Iterator<Item = &'a Project>
{
    let mut workspace = Workspace {
        tasks: HashMap::new(),
        build_envs: HashMap::new(),
        tools: HashMap::new()
    };

    for project in all_projects {
        add_project_to_workspace(project, &file_providers, &mut workspace);
    }

    workspace
}
