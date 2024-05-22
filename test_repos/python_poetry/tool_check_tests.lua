tool {
    name = "docker",
    check = function (c)
        local res = cmd { "docker", "--version" }
        assert(res.status == 0, "docker command exited with status " .. res.status)
        assert(res.stdout:match("Docker version [^%s]+, build [^%s]+"),
            "docker version did not match: " .. res.stdout)
    end,
    action = { tool = "cmd", function (c)
        return c.tool.cmd { "docker", table.unpack(c.args) }
    end}
}

local poetry_container_name = "cobble_test_python_poetry_poetry_container"
env {
    name = "poetry_container",
    install = {
        { tool = "docker", function (c)
            local success, res = pcall(c.tool.docker,  { "inspect", poetry_container_name })
            if success then
                return
            end

            c.tool.docker {
                "run", "-d", "--name", poetry_container_name, "python",
                "bash", "-c", "while true; do sleep 1d ; done"
            }
            c.tool.docker {
                "exec", poetry_container_name,
                "python", "-m", "pip", "install", "pipx"
            }
            -- TODO: Build image with a dockerfile instead of running a perpetual
            -- container
        end}
    },
    clean = {
        { tool = "docker", "stop", poetry_container_name },
        { tool = "docker", "rm", poetry_container_name }
    },
    action = { tool = "docker", "exec", poetry_container_name}
}
