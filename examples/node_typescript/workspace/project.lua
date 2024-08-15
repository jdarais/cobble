local tblext = require("tblext")
local cmd = require("cmd")
local version = require("version")

tool {
    name = "npm",
    check = function (c)
        local npm_cmd = PLATFORM.os_family == "windows" and { "powershell", "npm" } or { "npm" }
        local npm_version_result = cmd(tblext.extend(npm_cmd, { "--version" }))
        assert(version(npm_version_result.stdout) >= "10")
    end,
    action = function (c)
        local npm_cmd = PLATFORM.os_family == "windows" and { "powershell", "npm" } or { "npm" }
        return c.tool.cmd(tblext.extend(npm_cmd, c.args))
    end
}
