

external_tool {
    name = "poetry",
    action = { tool = "cmd", "poetry" }
}

task {
    name = "poetry_lock",
    deps = { files = { "pyproject.toml" } },
    artifacts = { "poetry.lock" },
    actions = {
        { tool = "poetry", "lock" }
    },
}

build_env {
    name = "poetry_env",
    install = {
        { tool = "poetry", "install" }
    },
    deps = {
        files = { "poetry.lock" }
    },
    action = { tool = "poetry", "run" }
}

task {
    name = "lint",
    actions = {
        { env = "poetry_env", "python", "-m", "pylint", "python_poetry/" }
    }
}


