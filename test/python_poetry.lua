

local function python_poetry()

    env {
        name = "docker_poetry",
        install = { { 
            tool = "docker",
            function (c)
                local res = c.tool.docker {
                    "build",
                    "-f", WORKSPACE.dir .. "/test_repos/python_poetry/Dockerfile.poetry",
                    WORKSPACE.dir
                }
                local image_hash = res.stdout:match("Successfully built ([%x]+)")
                assert(image_hash, "Failed to read hash for built 'docker_poetry' image")
                return { image = image_hash }
            end
        } },
        deps = { files = { "../test_repos/python_poetry/Dockerfile.poetry" } },
        action = { tool = "docker", function (c)
            local container_hash = c.tool.docker {
                "create", "--tty",
                "--mount", "type=bind,source=" .. WORKSPACE.dir .. "/test_repos/python_poetry/,target=/repo/",
                "--workdir", "/repo",
                c.tasks.install.image,
                table.unpack(c.args)
            }.stdout:match("[%x]+")
            local rm_on_exit = on_scope_exit(function() c.tool.docker { "rm", container_hash } end)
            c.tool.docker { "start", "--attach", container_hash }

            local cp_files = (c.args.cp_files or {})
            for i, f in ipairs(cp_files) do
                c.tool.docker { "cp", "/repo/"..f, WORKSPACE.dir .. "/" .. c.project.dir .. "/" .. f }
            end
        end }
    }


    task {
        name = "poetry_tool_check",
        actions = { { env = "docker_poetry", "cobl", "tool", "check", "poetry" } }
    }

end

return python_poetry
