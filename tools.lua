tool {
    name = "docker",
    check = function (c)
        local res = cmd { "docker", "--version" }
        assert(res.status == 0, "docker command exited with status " .. res.status)
        assert(res.stdout:match("Docker version [^%s]+, build [^%s]+"),
            "docker version did not match: " .. res.stdout)
    end,
    action = function (c) return c.tool.cmd(extend({"docker"}, c.args)) end
}

tool {
    name = "cargo",
    action = function (c) return c.tool.cmd(extend({"cargo"}, c.args)) end
}

tool {
    name = "git",
    action = function (c) return c.tool.cmd(extend({"git"}, c.args)) end
}
