local tblext = require("tblext")

tool {
    name = "yarn",
    action = function(c)
        local yarn_cmd = PLATFORM.os_family == "windows" and { "powershell", "yarn" } or { "yarn" }
        return c.tool.cmd(tblext.extend(yarn_cmd, c.args))
    end
}

tool {
    name = "npm",
    action = function (c)
        local npm_cmd = PLATFORM.os_family == "windows" and { "powershell", "npm" } or { "npm" }
        return c.tool.cmd(tblext.extend(npm_cmd, c.args))
    end
}
