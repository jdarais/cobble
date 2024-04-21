use std::{collections::HashMap, path::Path};

use crate::datamodel::{BuildEnv, Project, Task};





pub enum WorkspaceTargetRef<'a> {
    Project(&'a Project),
    Task(&'a Task),
    BuildEnv(&'a BuildEnv)
}

pub fn find_project_with_dir<'a, P>(all_projects: P, workspace_dir: &Path, project_dir: &Path) -> Option<&'a Project>
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

pub fn get_all_project_targets<'a, P>(all_projects: P, workspace_dir: &Path, project_dir: &Path) -> HashMap<&'a str, WorkspaceTargetRef<'a>>
    where P: Iterator<Item = &'a Project>
{
    let target_project_dir = workspace_dir.join(project_dir);
    let mut targets: HashMap<&'a str, WorkspaceTargetRef<'a>> = HashMap::new();

    for project in all_projects {
        let cur_project_dir = workspace_dir.join(project.path.as_path());
        if cur_project_dir.starts_with(target_project_dir.as_path()) {
            targets.insert(project.name.as_str(), WorkspaceTargetRef::Project(project));

            for env in project.build_envs.iter() {
                targets.insert(env.name.as_str(), WorkspaceTargetRef::BuildEnv(env));
            }

            for task in project.tasks.iter() {
                targets.insert(task.name.as_str(), WorkspaceTargetRef::Task(task));
            }
        }
    }

    targets
}
