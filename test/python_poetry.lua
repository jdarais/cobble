

local function python_poetry()
    task {
        name = "calc_python_poetry_test_repo_files",
        actions = { function (c)
            return {
                files = iter(ipairs(fs.glob(c.project.dir, "../test_repos/python_poetry/**/*")))
                    :filter(function (i, f) return not f:match(".mypy_cache") end)
                    :to_table()
            }
        end }
    }

    task {
        name = "python_poetry_image",
        actions = { { tool = "docker", "build", "-f", "python_poetry.Dockerfile", "-t", "local/cobble_test_python_poetry", ".." } },
        deps = {
            files = { "python_poetry.Dockerfile", "../.dockerignore", "../target/release/cobl" },
            calc = { "calc_python_poetry_test_repo_files" }
        }
    }

    task {
        name = "poetry_tool_check",
        actions = { { tool = "docker", "run", "--rm", "local/cobble_test_python_poetry", "cobl", "tool", "check", "poetry" } },
        deps = { tasks = { "python_poetry_image" } }
    }

end

return python_poetry
