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
