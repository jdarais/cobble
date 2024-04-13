
define_build_env(
    "name": "poetry",
    "install_task": {
        "actions": {
            { "poetry", "lock" },
            { "poetry", "install" }
        },
        "dependencies": {
            "files": { "pyproject.toml", "poetry.lock" }
        }
    }
)


