use std::collections::{HashMap, HashSet};

use crate::{datamodel::Dependency, workspace::query::WorkspaceTargetRef};


pub enum ExecutionGraphError {
    Cycle(String),
    TargetLookupError(String)
}

pub fn compute_order_graph_for_target<'a>(
    all_targets: &HashMap<&'a str, WorkspaceTargetRef<'a>>,
    file_providers: &HashMap<&'a str, &'a str>,
    target_name: &'a str,
    visit_history: &mut HashSet<&'a str>,
    order_graph: &mut HashMap<&'a str, Vec<&'a str>>
) -> Result<(), ExecutionGraphError> {
    if visit_history.contains(target_name) {
        return Err(ExecutionGraphError::Cycle(String::from(target_name)));
    }

    if order_graph.contains_key(target_name) {
        return Ok(());
    }

    visit_history.insert(target_name);
    let target = all_targets.get(target_name)
        .ok_or_else(|| ExecutionGraphError::TargetLookupError(String::from(target_name)))?;

    let mut task_deps: Vec<&'a str> = Vec::new();
    match target {
        WorkspaceTargetRef::Project(p) => {
            for task in p.tasks.iter() {
                task_deps.push(task.name.as_str());
                compute_order_graph_for_target
            (all_targets, file_providers, task.name.as_str(), visit_history, order_graph)?;
            }
        },
        WorkspaceTargetRef::Task(t) => {
            for dep in t.deps.iter() {
                match dep {
                    Dependency::File(f_dep) => {
                        let file_producer_opt = file_providers.get(f_dep.as_str());
                        if let Some(file_producer) = file_producer_opt {
                            task_deps.push(file_producer);
                            compute_order_graph_for_target
                        (all_targets, file_providers, file_producer, visit_history, order_graph)?;
                        }
                    },
                    Dependency::Task(t_dep) => {
                        task_deps.push(t_dep.as_str());
                        compute_order_graph_for_target
                    (all_targets, file_providers, t_dep.as_str(), visit_history, order_graph)?;
                    },
                    Dependency::Calc(c_dep) => {
                        /* Ignore calc deps for now */
                    }
                }
            }
        },
        WorkspaceTargetRef::BuildEnv(e_dep) => {
            for dep in e_dep.deps.iter() {
                match dep {
                    Dependency::File(f_dep) => {
                        let file_producer_opt = file_providers.get(f_dep.as_str());
                        if let Some(file_producer) = file_producer_opt {
                            task_deps.push(file_producer);
                            compute_order_graph_for_target
                        (all_targets, file_providers, file_producer, visit_history, order_graph)?;
                        }
                    },
                    Dependency::Task(t_dep) => {
                        task_deps.push(t_dep.as_str());
                        compute_order_graph_for_target
                    (all_targets, file_providers, t_dep.as_str(), visit_history, order_graph)?;
                    },
                    Dependency::Calc(c_dep) => {
                        /* Ignore calc deps for now */
                    }
                }
            }
        }
    }

    visit_history.remove(target_name);

    Ok(())
}
