local tblext = require("tblext")
local cmd = require("cmd")
local maybe = require("maybe")
local version = require("version")

tool {
    name = "docker",
    check = function (c)
        local res = cmd { "docker", "--version" }
        assert(res.status == 0, "docker command exited with status " .. res.status)
        local docker_version = res.stdout:match("Docker version ([^%s]+), build [^%s]+")
        assert(docker_version, "Unable to get docker version from 'docker --version' command output: " .. res.stdout)

        local min_version = c.vars.docker.min_version
        if min_version ~= nil then
            assert(version(docker_version) > version(min_version), "Docker version must be at least " .. min_version .. ". Found version " .. docker_version)
        end
    end,
    action = function (c) return c.tool.cmd(tblext.extend({"docker"}, c.args)) end,
    deps = {
        vars = { "docker.min_version" }
    }
}

tool { name = "cargo", action = {"cargo"} }

tool { name = "git", action = {"git"} }

tool { name = "wsl", action = {"wsl"} }

tool { name = "python", action = {"python"} }
