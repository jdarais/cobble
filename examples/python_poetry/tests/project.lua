local path = require("path")
local iter = require("iter")

task {
    name = "calc_python_poetry_test_repo_files",
    always_run = true,
    actions = { function (c)
        return {
            files = iter(ipairs(path.glob(c.project.dir, "../workspace/**/*")))
                :filter(function (i, f) return not f:match(".mypy_cache") end)
                :filter(function (i, f) return not f:match(".venv") end)
                :filter(function (i, f) return path.is_file(f) end)
                :to_table()
        }
    end }
}

task {
    name = "python_poetry_image",
    actions = { { tool = "docker", "build", "-f", "python_poetry.Dockerfile", "-t", "local/cobble_test_python_poetry", "../../.." } },
    deps = {
        files = { "python_poetry.Dockerfile", "../../../.dockerignore", "../../../target/release/cobl" },
        calc = { "calc_python_poetry_test_repo_files" }
    }
}

task {
    name = "test_poetry_tool_check",
    default = true,
    actions = { { tool = "docker", "run", "--rm", "local/cobble_test_python_poetry", "cobl", "tool", "check", "poetry" } },
    deps = { tasks = { "python_poetry_image" } }
}

task {
    name = "test_lint",
    default = true,
    actions = { { tool = "docker", "run", "--rm", "local/cobble_test_python_poetry", "cobl", "run", "lint" } },
    deps = { tasks = { "python_poetry_image" } }
}





