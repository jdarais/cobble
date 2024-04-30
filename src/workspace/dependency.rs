use std::{borrow::Cow, collections::{HashMap, HashSet}, convert::AsRef, fmt, sync::Arc};

use crate::workspace::query::{Workspace, Task};

#[derive(Debug)]
pub enum ExecutionGraphError {
    CycleError(String),
    TaskLookupError(String),
    DuplicateFileProviderError{provider1: String, provider2: String, file: String}
}

impl fmt::Display for ExecutionGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ExecutionGraphError::*;
        match self {
            CycleError(task) => write!(f, "Cycle detected at {}", task),
            TaskLookupError(task) => write!(f, "Task not found: {}", task),
            DuplicateFileProviderError{provider1, provider2, file} =>
                write!(f, "Multiple providers found for file dependency {}: {}, {}", file, provider1, provider2)
        }
    }
}

fn compute_file_providers<'a>(workspace: &'a Workspace) -> Result<HashMap<&'a String, &'a String>, ExecutionGraphError> {
    let mut file_providers: HashMap<&'a String, &'a String> = HashMap::new();

    for (task_name, task) in workspace.tasks.iter() {
        for artifact in task.artifacts.iter() {
            file_providers.insert(&artifact.filename, &task_name);
        }
    }

    Ok(file_providers)
}

fn materialize_dependencies_in_subtree(workspace: &mut Workspace, task_name: &str, file_providers: &HashMap<&String, &String>) -> Result<bool, ExecutionGraphError> {
    let task = workspace.tasks.get(task_name)
        .ok_or_else(|| ExecutionGraphError::TaskLookupError(task_name.to_owned()))?
        .clone();

    let mut updated_task: Cow<Task> = Cow::Borrowed(task.as_ref());

    for file_dep in task.file_deps.iter() {
        if let Some(&provider) = file_providers.get(file_dep) {
            if !updated_task.task_deps.contains(provider) {
                updated_task.to_mut().task_deps.push(provider.to_owned());
            }
        }
    }

    for _calc_dep in task.calc_deps.iter() {
        // Ignore calc deps for now
    }

    if let Cow::Owned(owned_updated_task) = updated_task {
        workspace.tasks.insert(task_name.to_owned(), Arc::new(owned_updated_task));
    }

    for dep in task.task_deps.iter() {
        materialize_dependencies_in_subtree(workspace, dep.as_str(), file_providers)?;
    }

    Ok(false)
}


pub fn compute_forward_edges<'a>(workspace: &'a Workspace) -> HashMap<&'a str, Vec<&'a str>> {
    let mut forward_edges: HashMap<&'a str, HashSet<&'a str>> = HashMap::new();

    for (task_name, task) in workspace.tasks.iter() {
        for task_dep in task.task_deps.iter() {
            match forward_edges.get_mut(task_dep.as_str()) {
                Some(task_dep_forward_edges) => { task_dep_forward_edges.insert(task_name.as_str()); },
                None => {
                    let mut task_dep_forward_edges: HashSet<&'a str> = HashSet::new();
                    task_dep_forward_edges.insert(task_name.as_str());
                    forward_edges.insert(task_dep.as_str(), task_dep_forward_edges);
                }
            }
        }
    }

    forward_edges.into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect()
}

#[cfg(test)]
mod tests {
   
}
