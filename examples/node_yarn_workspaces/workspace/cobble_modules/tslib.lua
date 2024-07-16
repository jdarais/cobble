local path = require("path")
local tblext = require("tblext")
local iter = require("iter")
local json = require("json")
local maybe = require("maybe")

local exports = {}

function exports.yarn_typescript_lib ()
    env {
        name = "yarn",
        setup_task = {
            actions = { { tool = "yarn", "install" } },
            deps = { files = { "package.json" } }
        },
        action = { tool = "yarn", function (c) return c.tool.yarn (tblext.extend({ "run" }, c.args)) end }
    }

    task {
        name = "calc_build_inputs",
        env = "yarn",
        actions = {
            function (c)
                local tsc_config_result = c.env.yarn { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)
                return tsc_config["files"]
            end
        }
    }

    task {
        name = "calc_build_outputs",
        env = "yarn",
        actions = {
            function (c)
                local tsc_config_result = c.env.yarn { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)

                local root_dir = maybe(tsc_config)["compilerOptions"]["rootDir"].value or "./"
                local out_dir = maybe(tsc_config)["compilerOptions"]["outDir"].value or "./"

                local out_files = iter(ipairs(tsc_config["files"])):map(function(i, f) return i, f:gsub("^"..root_dir, out_dir) end):to_table()
                print(tblext.format(out_files))
                return out_files
            end
        }
    }

    task {
        name = "build",
        env = "yarn",
        actions = {{ env = "yarn", "tsc" }},
        deps = { files = src_files },
    }


end

return exports
