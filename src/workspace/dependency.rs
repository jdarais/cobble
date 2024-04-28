use std::{borrow::Cow, collections::{HashMap, HashSet}, convert::AsRef, fmt, hash::Hash, path::Path, sync::Arc};

use crate::{datamodel::{Dependency, Project}, workspace::query::{Workspace, WorkspaceTarget}};

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

fn compute_file_providers<'a>(workspace: &'a Workspace) -> Result<HashMap<&'a String, &'a String>, ExecutionGraphError> {
    let mut file_providers: HashMap<&'a String, &'a String> = HashMap::new();

    for (target_name, target) in workspace.targets.iter() {
        for artifact in target.artifacts.iter() {
            file_providers.insert(&artifact.filename, &target_name);
        }
    }

    Ok(file_providers)
}

fn materialize_dependencies_in_subtree(workspace: &mut Workspace, target_name: &str, file_providers: &HashMap<&String, &String>) -> Result<bool, ExecutionGraphError> {
    let target = workspace.targets.get(target_name)
        .ok_or_else(|| ExecutionGraphError::TargetLookupError(target_name.to_owned()))?
        .clone();

    let mut updated_target: Cow<WorkspaceTarget> = Cow::Borrowed(target.as_ref());

    for file_dep in target.file_deps.iter() {
        if let Some(&provider) = file_providers.get(file_dep) {
            if !updated_target.target_deps.contains(provider) {
                updated_target.to_mut().target_deps.push(provider.to_owned());
            }
        }
    }

    for calc_dep in target.calc_deps.iter() {
        // Ignore calc deps for now
    }

    if let Cow::Owned(owned_updated_target) = updated_target {
        workspace.targets.insert(target_name.to_owned(), Arc::new(owned_updated_target));
    }

    for dep in target.target_deps.iter() {
        materialize_dependencies_in_subtree(workspace, dep.as_str(), file_providers)?;
    }

    Ok(false)
}


pub fn compute_forward_edges<'a>(workspace: &'a Workspace) -> HashMap<&'a str, Vec<&'a str>> {
    let mut forward_edges_sets: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();

    for (target_name, target) in workspace.targets.iter() {
        for target_dep in target.target_deps.iter() {
            match forward_edges_sets.get_mut(target_dep.as_str()) {
                Some(back_node_forward_edges_set) => { back_node_forward_edges_set.insert(target_dep.as_str()); },
                None => {
                    let mut back_node_forward_edges_set: HashSet<&'a str> = HashSet::new();
                    back_node_forward_edges_set.insert(target_dep.as_str());
                    forward_edges_sets.insert(target_name.as_str(), back_node_forward_edges_set);
                }
            }
        }
    }

    forward_edges_sets.into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect()
}

#[cfg(test)]
mod tests {
   
}
