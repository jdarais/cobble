


pub enum Action {
    Cmd(Vec<String>),
    Func(String)
}

pub enum Dependency {
    File(String),
    Task(String)
}

pub struct Artifact {
    filename: String
}


pub struct Task {
    build_env_name: String,
    actions: Vec<Action>,
    dependencies: Vec<Dependency>,
    artifacts: Vec<Artifact>
}