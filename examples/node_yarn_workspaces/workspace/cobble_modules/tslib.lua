local path = require("path")
local tblext = require("tblext")
local iter = require("iter")
local json = require("json")
local maybe = require("maybe")

local exports = {}

function exports.yarn_typescript_lib ()
    env {
        name = "npm",
        setup_task = {
            actions = { { tool = "npm", "install" } },
            deps = { files = { "package.json" } }
        },
        action = { tool = "npm", function (c) return c.tool.npm (tblext.extend({ "exec", "--" }, c.args)) end }
    }

    task {
        name = "calc_build_inputs",
        env = "npm",
        actions = {
            function (c)
                local tsc_config_result = c.env.npm { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)
                return tsc_config["files"]
            end
        }
    }

    task {
        name = "calc_build_outputs",
        env = "npm",
        always_run = true,
        actions = {
            function (c)
                local tsc_config_result = c.env.npm { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)

                local root_dir = maybe(tsc_config)["compilerOptions"]["rootDir"].value or "./"
                local out_dir = maybe(tsc_config)["compilerOptions"]["outDir"].value or "./"

                local out_files = iter(ipairs(tsc_config["files"])):map(function(i, f) return i, f:gsub("^"..root_dir, out_dir) end):to_table()
                return out_files
            end
        }
    }

    task {
        name = "build",
        env = "npm",
        actions = {{ env = "npm", "tsc" }},
        deps = { calc = { "calc_build_inputs" } },
        artifacts = { calc = { "calc_build_outputs" } }
    }


end

return exports
