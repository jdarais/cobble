mod cobble;

use std::path::Path;

use cobble::workspace::{
    extract_project_defs, find_nearest_workspace_dir_from, init_lua_for_project_config, parse_workspace_config_file, process_project_file, WORKSPACE_CONFIG_FILE_NAME
};

use cobble::lua_env::create_lua_env;


fn run_from_dir(path: &Path) {
    let workspace_dir = find_nearest_workspace_dir_from(path).unwrap();

    let workspace_config = parse_workspace_config_file(workspace_dir.join(WORKSPACE_CONFIG_FILE_NAME).as_path()).unwrap();

    let project_def_lua = create_lua_env(workspace_dir.as_path()).unwrap();

    init_lua_for_project_config(&project_def_lua, workspace_dir.as_path()).unwrap();

    for project_dir in &workspace_config.root_projects {
        process_project_file(&project_def_lua, project_dir.as_str(), workspace_dir.as_path()).unwrap();
    }

    let projects = extract_project_defs(&project_def_lua).unwrap();

    println!("Projects:");
    for (name, proj) in projects {
        println!("\"{}\" = {}", name, proj);
    }

    let package_path: String = project_def_lua.load("package.path").eval().unwrap();
    println!("package.path={}", package_path.as_str());
}

fn main() {
    let cwd = std::env::current_dir().expect("was run from a directory");

    run_from_dir(cwd.as_path())
}
