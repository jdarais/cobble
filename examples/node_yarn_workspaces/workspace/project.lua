local tblext = require("tblext")

tool {
    name = "yarn",
    action = function(c)
        local yarn_cmd = PLATFORM.os_family == "windows" and { "powershell", "yarn" } or { "yarn" }
        return c.tool.cmd(tblext.extend(yarn_cmd, c.args))
    end
}