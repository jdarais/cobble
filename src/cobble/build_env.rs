use crate::cobble::task::Task;

pub struct BuildEnv {
    install_task: Task,
    run_command: String
}