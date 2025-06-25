local path = require("path")
local tblext = require("tblext")

local ws_dir = WORKSPACE.dir

local module = {}

if PLATFORM.os == "windows" then
    module.python_exe = "python.exe"
    module.venv_python_path = path.join("Scripts", "python.exe")
else
    module.python_exe = "python3"
    module.venv_python_path = path.join("bin", "python")
end

function module.python_project(args)
    -- local_packages should point to tasks that have a "local_requirements" property in their output, containing
    -- a list of requirements to add to requirements.txt files used for building venvs in this project.
    local local_packages = args.local_packages or {}

    local constraints_file = {
        files = { constraints_file = args.constraints_file }
    }

    if type(args.constraints_file_calc) == "string" then
        task {
            name = "constraints_file_calc",
            deps = { tasks = { args.constraints_file_calc } },
            actions = {
                function (c)
                    return { files = { constraints_file = c.tasks[1].output } }
                end
            }
        }

        constraints_file.calc = { "constraints_file_calc" }
    end

    -- task {
    --     name = "pyproject_toml",
    --     artifacts = { files = { "pyproject.toml" } },
    --     deps = {
    --         files = { "pyproject.source.toml" },
    --         vars = { "project.version" }
    --     },
    --     actions = {

    --     }
    -- }

    task {
        name = "venv_requirements_file",
        deps = { tasks = tblext.extend({}, local_packages) },
        artifacts = { files = { requirements = "requirements.venv.txt" } },
        actions = {
            function (c)
                local requirements = {}
                for k, task in pairs(c.tasks) do
                    if task.output.local_requirements then
                        table.insert(requirements, task.output.local_requirements)
                    end
                end
                -- We'll always add our own project in editable mode
                table.insert(requirements, "-e .")

                io.open(path.join(c.project.dir, "requirements.venv.txt"), "w")
                :write(table.concat(requirements, "\n"))
                :close()
            end
        }
    }

    env {
        name = "python_venv",
        setup_task = {
            deps = {
                files = tblext.extend({ requirements = "requirements.venv.txt" }, constraints_file.files or {}),
                calc = tblext.extend({}, constraints_file.calc or {})
            },
            actions = {
                { tool = "python", "-m", "venv", ".venv" },
                function (c)
                    local install_command = {
                        path.join(".venv", module.venv_python_path), "-m", "pip", "install",
                        "-r", path.join(ws_dir, c.files.requirements.path),
                    }
                    if c.files.constraints_file then
                        tblext.extend(install_command, { "-c", path.join(ws_dir, c.files.constraints_file.path) })
                    end
                    c.tool.cmd(install_command)
                end
            }
        },
        action = {
            tool = "python",
            function (c)
                c.tool.cmd(tblext.extend({ path.join(".venv", module.venv_python_path) }, c.args))
            end
        }
    }

end










return module
