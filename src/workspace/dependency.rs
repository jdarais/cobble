use std::{collections::{HashMap, HashSet}, convert::AsRef, fmt, hash::Hash, path::Path};

use crate::{datamodel::{Dependency, Project}, workspace::query::WorkspaceTargetRef};

#[derive(Debug)]
pub enum ExecutionGraphError {
    CycleError(String),
    TargetLookupError(String),
    DuplicateFileProviderError{provider1: String, provider2: String, file: String}
}

impl fmt::Display for ExecutionGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecutionGraphError::*;
        match self {
            CycleError(target) => write!(f, "Cycle detected at {}", target),
            TargetLookupError(target) => write!(f, "Target not found: {}", target),
            DuplicateFileProviderError{provider1, provider2, file} =>
                write!(f, "Multiple providers found for file dependency {}: {}, {}", file, provider1, provider2)
        }
    }
}

fn compute_file_providers<'a, T>(targets: T, all_targets: &HashMap<&'a str, WorkspaceTargetRef<'a>>) -> Result<HashMap<&'a str, &'a str>, ExecutionGraphError>
    where T: Iterator<Item = &'a str>
{
    let mut file_providers: HashMap<&'a str, &'a str> = HashMap::new();

    for target_name in targets {
        let target = match all_targets.get(target_name) {
            Some(t) => t,
            None => { return Err(ExecutionGraphError::TargetLookupError(String::from(target_name))); }
        };

        if let WorkspaceTargetRef::Task(task) = target {
            for artifact in task.artifacts.iter() {
                if file_providers.contains_key(artifact.filename.as_str()) {
                    return Err(ExecutionGraphError::DuplicateFileProviderError {
                        provider1: String::from(file_providers[artifact.filename.as_str()]),
                        provider2: task.name.clone(),
                        file: artifact.filename.clone()
                    });
                }
                file_providers.insert(artifact.filename.as_str(), task.name.as_str());
            }
        }
    }

    Ok(file_providers)
}

pub struct ExecutionGraph<'a> {
    pub back_edges: HashMap<&'a str, Vec<&'a str>>,
    pub forward_edges: HashMap<&'a str, Vec<&'a str>>,
    pub required_files: HashSet<&'a str>,
    pub required_tools: HashSet<&'a str>
}

pub fn compute_execution_graph_for_targets<'a>(targets: Vec<&'a str>, all_targets: &HashMap<&'a str, WorkspaceTargetRef<'a>>) -> Result<ExecutionGraph<'a>, ExecutionGraphError> {
    let mut graph = ExecutionGraph {
        back_edges: HashMap::new(),
        forward_edges: HashMap::new(),
        required_files: HashSet::new(),
        required_tools: HashSet::new()
    };

    let file_providers: HashMap<&'a str, &'a str> = compute_file_providers(targets.iter().copied(), all_targets)?;

    for target in targets {
        compute_back_edges_for_target(target, all_targets, &file_providers, &mut graph, &mut HashSet::new())?;
    }

    compute_forward_edges(&mut graph);

    Ok(graph)
}

struct DependencyResolution<'a> {
    pub targets: Vec<&'a str>,
    pub required_files: Vec<&'a str>
}

fn resolve_dependency<'a>(dep: &'a Dependency, file_providers: &HashMap<&'a str, &'a str>) -> DependencyResolution<'a> {
    match dep {
        Dependency::File(f_dep) => {
            match file_providers.get(f_dep.as_str()).map(|&f| f) {
                Some(file_producer) => DependencyResolution { targets: vec![file_producer], required_files: Vec::new() },
                None => DependencyResolution { targets: Vec::new(), required_files: vec![f_dep] }
            }
        },
        Dependency::Task(t_dep) => DependencyResolution { targets: vec![t_dep.as_str()], required_files: Vec::new() },
        // Ignore Calc deps for now
        Dependency::Calc(_c_dep) => DependencyResolution { targets: Vec::new(), required_files: Vec::new() }
    }
}

fn compute_back_edges_for_target<'a>(
    target_name: &'a str,
    all_targets: &HashMap<&'a str, WorkspaceTargetRef<'a>>,
    file_providers: &HashMap<&'a str, &'a str>,
    graph: &mut ExecutionGraph<'a>,
    visit_history: &mut HashSet<&'a str>,
) -> Result<(), ExecutionGraphError> {
    if visit_history.contains(target_name) {
        return Err(ExecutionGraphError::CycleError(String::from(target_name)));
    }

    if graph.back_edges.contains_key(target_name) {
        return Ok(());
    }

    visit_history.insert(target_name);
    let target = all_targets.get(target_name)
        .ok_or_else(|| ExecutionGraphError::TargetLookupError(String::from(target_name)))?;

    let mut task_deps: HashSet<&'a str> = HashSet::new();
    match target {
        WorkspaceTargetRef::Project(p) => {
            for task in p.tasks.iter() {
                task_deps.insert(task.name.as_str());
                compute_back_edges_for_target(task.name.as_str(), all_targets, file_providers, graph, visit_history)?;
            }
            // Add all subprojects
            for t in all_targets.values() {
                if let WorkspaceTargetRef::Project(subproject) = t {
                    if subproject.name.len() != p.name.len() && subproject.name.starts_with(p.name.as_str()) {
                        task_deps.insert(subproject.name.as_str());
                        compute_back_edges_for_target(subproject.name.as_str(), all_targets, file_providers, graph, visit_history)?;
                    }
                }
            }
        },
        WorkspaceTargetRef::Task(t) => {
            for dep in t.deps.iter() {
                let resolved_dep = resolve_dependency(dep, file_providers);
                for dep_target in resolved_dep.targets {
                    task_deps.insert(dep_target);
                    compute_back_edges_for_target(dep_target, all_targets, file_providers, graph, visit_history)?;
                }
                for file in resolved_dep.required_files {
                    graph.required_files.insert(file);
                }
            }
            for act in t.actions.iter() {
                for (_, env) in act.build_envs.iter() {
                    task_deps.insert(env);
                }
                for (_, tool) in act.tools.iter() {
                    graph.required_tools.insert(tool);
                }
            }
            if let Some((_, env)) = t.build_env.as_ref() {
                task_deps.insert(env);
            }
        },
        WorkspaceTargetRef::BuildEnv(e_dep) => {
            for dep in e_dep.deps.iter() {
                // compute_back_edges_for_dependency(dep, all_targets, file_providers, &mut task_deps, graph, visit_history)?;
                let resolved_dep = resolve_dependency(dep, file_providers);
                for dep_target in resolved_dep.targets {
                    task_deps.insert(&dep_target);
                    compute_back_edges_for_target(dep_target, all_targets, file_providers, graph, visit_history)?;
                }
                for file in resolved_dep.required_files {
                    graph.required_files.insert(file);
                }
            }
        }
    }

    graph.back_edges.insert(target_name, task_deps.into_iter().collect());
    visit_history.remove(target_name);

    Ok(())
}

fn compute_forward_edges<'a>(graph: &mut ExecutionGraph<'a>) {
    let back_edges = &graph.back_edges;
    let mut forward_edges_sets: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();

    for (node, back_edges) in back_edges.iter() {
        for back_edge in back_edges {
            match forward_edges_sets.get_mut(back_edge) {
                Some(back_node_forward_edges_set) => { back_node_forward_edges_set.insert(node); },
                None => {
                    let mut back_node_forward_edges_set: HashSet<&'a str> = HashSet::new();
                    back_node_forward_edges_set.insert(node);
                    forward_edges_sets.insert(back_edge, back_node_forward_edges_set);
                }
            }
        }
    }

    for (node, forward_edges_set) in forward_edges_sets.into_iter() {
        graph.forward_edges.insert(node, forward_edges_set.into_iter().collect());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    use crate::{datamodel::{Action, ActionCmd, BuildEnv, ExternalTool, Project, Task}, workspace::{dependency::compute_file_providers, query::get_all_targets}};

    #[test]
    fn test_compute_small_graph() {
        let project: Project = Project {
            name: String::from("/"),
            path: PathBuf::from("."),
            build_envs: vec![
                BuildEnv {
                    name: String::from("/testenv"),
                    install: Vec::new(),
                    deps: vec![ Dependency::File(String::from("./envdep1"))],
                    action: Action {
                        build_envs: HashMap::new(),
                        tools: HashMap::new(),
                        cmd: ActionCmd::Cmd(vec![String::from("arg1"), String::from("arg2")])
                    }
                }
            ],
            tools: vec![
                ExternalTool {
                    name: String::from("testtool"),
                    install: None,
                    check: None,
                    action: Action {
                        build_envs: HashMap::new(),
                        tools: HashMap::new(),
                        cmd: ActionCmd::Cmd(vec![String::from("arg1")])
                    }
                }
            ],
            tasks: vec![
                Task {
                    name: String::from("/task1"),
                    build_env: None,
                    actions: Vec::new(),
                    deps: vec![Dependency::Task(String::from("/task2"))],
                    artifacts: Vec::new()
                },
                Task {
                    name: String::from("/task2"),
                    build_env: None,
                    actions: vec![
                        Action {
                            build_envs: vec![(String::from("testenv"), String::from("/testenv"))].into_iter().collect(),
                            tools: HashMap::new(),
                            cmd: ActionCmd::Cmd(vec![String::from("envarg1")])
                        },
                        Action {
                            build_envs: HashMap::new(),
                            tools: vec![(String::from("testtool"), String::from("testtool"))].into_iter().collect(),
                            cmd: ActionCmd::Cmd(vec![String::from("toolarg1")])
                        }
                    ],
                    deps: vec![Dependency::File(String::from("./taskdep1"))],
                    artifacts: Vec::new()
                }
            ],
            child_project_names: Vec::new()
        };

        let projects = vec![project];

        let targets = get_all_targets(projects.iter());

        let graph = compute_execution_graph_for_targets(targets.keys().copied().collect(), &targets).unwrap();
        assert_eq!(graph.back_edges["/task1"], vec!["/task2"]);

        let mut task_2_forward_edges = graph.forward_edges["/task2"].clone();
        task_2_forward_edges.sort();
        assert_eq!(task_2_forward_edges, vec!["/", "/task1"]);

        assert_eq!(graph.back_edges["/task2"], vec!["/testenv"]);
        assert_eq!(graph.required_files, vec!["./envdep1", "./taskdep1"].into_iter().collect::<HashSet<&str>>());
    }
}
