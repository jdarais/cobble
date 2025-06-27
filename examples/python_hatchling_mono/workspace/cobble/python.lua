local path = require("path")
local tblext = require("tblext")
local toml = require("toml")
local maybe = require("maybe")

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

    local build_venv_env = args.build_venv_env

    if type(args.constraints_file_calc) == "string" then
        task {
            name = "constraints_file_calc",
            visible = false,
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
        visible = false,
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
            description = "Sets up the virtual environment for the project",
            deps = tblext.extend(
                { files = { requirements = "requirements.venv.txt" } },
                constraints_file,
                { deep = true }
            ),
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

    task {
        name = "calc_package_sdist_name",
        visible = false,
        deps = { files = { pyproject_toml = "pyproject.toml" } },
        actions = {
            function (c)
                pyproject = toml.load(c.files.pyproject_toml.path)
                local name = pyproject.project.name
                local version = pyproject.project.version
                return name.."-"..version..".tar.gz"
            end
        }
    }

    task {
        name = "calc_package_sdist",
        visible = false,
        deps = { tasks = { package_name = "calc_package_sdist_name" } },
        actions = {
            function (c) return { files = { sdist = path.join("dist", c.tasks.package_name.output) } } end
        }
    }

    task {
        name = "package_sdist",
        description = "Builds the sdist package. Returns requirements.txt entries for this package and all local requirements reported by dependencies.",
        deps = {
            files = { pyproject_toml = "pyproject.toml" },
            tasks = tblext.extend({ package_name = "calc_package_sdist_name" }, local_packages)
        },
        artifacts = {
            calc = { "calc_package_sdist" }
        },
        actions = {
            { env = { build = args.build_venv_env }, "-m", "build", "--sdist" },
            function (c)
                local local_requirements = {}
                for k, v in pairs(c.tasks) do
                    if type(c.tasks.local_requirements) == "table" then
                        for _, req in pairs(c.tasks.local_requirements) do
                            table.insert(local_requirements, req)
                        end
                    end
                end
                table.insert(local_requirements, path.join(ws_dir, c.project.dir, c.tasks.package_name.output))
                return { local_requirements = local_requirements }
            end
        }
    }

end










return module
