local path = require("path")
local tblext = require("tblext")
local iter = require("iter")
local json = require("json")
local maybe = require("maybe")

local exports = {}

function exports.npm_package ()
    env {
        name = "npm",
        setup_task = {
            actions = { { tool = "npm", "install" } },
            deps = { files = { "package.json" } }
        },
        action = { tool = "npm", function (c) return c.tool.npm (tblext.extend({ "exec", "--" }, c.args)) end }
    }

    task {
        name = "version",
        always_run = true,
        actions = {
            { env = "npm", "version" }
        }
    }

    task {
        name = "calc_package_dep_build_tasks",
        always_run = true,
        actions = {
            {
                tool = "npm",
                function (c)
                    local package_json = json.load(path.join(c.project.dir, "package.json"))
                    local deps = package_json["dependencies"] or {}

                    -- This task assumes that the npm workspace root is the same as the cobble workspace root
                    local workspace_deps_result = c.tool.npm { cwd = WORKSPACE.dir, "query", "--", ".workspace" }
                    local workspace_deps_list = json.loads(workspace_deps_result.stdout)
                    
                    local task_deps = { tasks = {} }

                    for i, ws_dep in ipairs(workspace_deps_list) do
                        if deps[ws_dep["name"]] ~= nil then
                            table.insert(task_deps.tasks, "["..ws_dep["path"].."]/build")
                        end
                    end

                    return task_deps
                end
            }
        }
    }
end

return exports
