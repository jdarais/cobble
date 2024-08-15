local path = require("path")
local tblext = require("tblext")
local iter = require("iter")
local json = require("json")
local maybe = require("maybe")
local script_dir = require("script_dir")

local npm = require("cobble_modules.npm")

local exports = {}

function exports.npm_typescript_lib ()
    if not PROJECT.build_envs["npm"] then
        npm.npm_package()
    end

    task {
        name = "calc_build_inputs",
        env = "npm_env",
        always_run = true,
        actions = {
            function (c)
                local tsc_config_result = c.env.npm_env { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)
                return { files = tsc_config["files"] }
            end
        }
    }

    task {
        name = "calc_build_outputs",
        env = "npm_env",
        always_run = true,
        actions = {
            function (c)
                local tsc_config_result = c.env.npm_env { "tsc", "--showConfig" }
                local tsc_config = json.loads(tsc_config_result.stdout)

                local root_dir = maybe(tsc_config)["compilerOptions"]["rootDir"].value or "./"
                local out_dir = maybe(tsc_config)["compilerOptions"]["outDir"].value or "./"

                local out_files = iter(ipairs(tsc_config["files"]))
                    :map(function(i, f) return i, f:gsub("^"..root_dir, out_dir) end)
                    :map(function(i, f) return i, f:gsub(".tsx?$", ".js") end)
                    :to_table()
                return out_files
            end
        }
    }

    task {
        name = "build",
        env = "npm_env",
        actions = {{ env = "npm_env", "tsc" }},
        deps = {
            files = { "tsconfig.json" },
            calc = { "calc_build_inputs", "calc_package_dep_build_tasks" }
        },
        artifacts = { calc = { "calc_build_outputs" } }
    }


end

return exports
