local path = require("path")
local tblext = require("tblext")
local toml = require("toml")
local maybe = require("maybe")

local ws_dir = WORKSPACE.dir

local module = {}

if PLATFORM.os == "windows" then
    module.venv_bin_path = "Scripts"
    module.python_exe = "python.exe"
else
    module.venv_bin_path = "bin"
    module.python_exe = "python3"
end

module.venv_python_path = path.join(module.venv_bin_path, module.python_exe)

function module.python_project(args)
    -- local_packages should point to tasks that have a "local_requirements" property in their output, containing
    -- a list of requirements to add to requirements.txt files used for building venvs in this project.
    local local_packages = args.local_packages or {}

    local constraints_file = {
        files = { constraints_file = args.constraints_file }
    }

    local build_venv = args.build_venv

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
                local args = {table.unpack(c.args)}
                local arg1 = table.remove(args, 1)
                c.tool.cmd(tblext.extend({ path.join(".venv", module.venv_bin_path, arg1) }, c.args))
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
            { env = build_venv, "python", "-m", "build", "--sdist" },
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

    task {
        name = "calc_package_wheel_name",
        visible = false,
        deps = { files = { pyproject_toml = "pyproject.toml" } },
        actions = {
            function (c)
                pyproject = toml.load(c.files.pyproject_toml.path)
                local name = pyproject.project.name
                local version = pyproject.project.version
                -- TODO: Make the file name components configurable
                return name.."-"..version.."-untagged.whl"
            end
        }
    }

    task {
        name = "calc_package_wheel",
        visible = false,
        deps = { tasks = { package_name = "calc_package_wheel_name" } },
        actions = {
            function (c) return { files = { wheel = path.join("dist", c.tasks.package_name.output) } } end
        }
    }

    task {
        name = "package_wheel",
        description = "Builds the wheel package. Returns requirements.txt entries for this package and all local\n"
                        .."requirements reported by dependencies. Since it's difficult to compute the name of the\n"
                        .."tagged wheel beforehand, 'package_wheel' produces an additional 'untagged' wheel file and\n"
                        .."declares that as its artifact.  It provides the tagged wheel name in its output as the\n"
                        .."'tagged_wheel_name' property",
        deps = {
            files = { pyproject_toml = "pyproject.toml" },
            tasks = tblext.extend({ package_name = "calc_package_wheel_name" }, local_packages)
        },
        artifacts = {
            calc = { "calc_package_wheel" }
        },
        actions = {
            {
                env = { build_venv = build_venv },
                function (c)
                    local build_res = c.env.build_venv { "python", "-m", "build", "--wheel" }
                    local wheel_name = build_res.stdout:match("Successfully built ([^%s]+)")
                    assert(wheel_name)

                    local cp_from = io.open(path.join(c.project.dir, "dist", wheel_name), "rb")
                    local cp_to = io.open(path.join(c.project.dir, "dist", c.tasks.package_name.output), "wb")
                    local data = ""
                    while data do
                        cp_to:write(data)
                        data = cp_from:read(1024)
                    end
                    cp_from:close()
                    cp_to:close()

                    local local_requirements = {}
                    for k, v in pairs(c.tasks) do
                        if type(c.tasks.local_requirements) == "table" then
                            for _, req in pairs(c.tasks.local_requirements) do
                                table.insert(local_requirements, req)
                            end
                        end
                    end
                    table.insert(local_requirements, path.join(ws_dir, c.project.dir, wheel_name))
                    return { local_requirements = local_requirements, tagged_wheel_name = wheel_name }
                end
            }
        }
    }

    task {
        name = "package_editable",
        description = "Acts as a 'package' that will be installed editable mode. Returns requirements.txt entries for this package and all local requirements reported by dependencies.",
        deps = {
            files = { pyproject_toml = "pyproject.toml" },
            tasks = tblext.extend({ package_name = "calc_package_wheel_name" }, local_packages)
        },
        artifacts = {
            calc = { "calc_package_wheel" }
        },
        actions = {
            function (c)
                local local_requirements = {}
                for k, v in pairs(c.tasks) do
                    if type(c.tasks.local_requirements) == "table" then
                        for _, req in pairs(c.tasks.local_requirements) do
                            table.insert(local_requirements, req)
                        end
                    end
                end
                table.insert(local_requirements, "-e "..path.join(ws_dir, c.project.dir))
                return { local_requirements = local_requirements }
            end
        }
    }

end










return module
