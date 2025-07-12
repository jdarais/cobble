local path = require("path")
local tblext = require("tblext")
local toml = require("toml")
local maybe = require("maybe")
local fs = require("fs")
local ordered_map = require("ordered_map")

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

local function find_source_deps_for_target(pyproject_toml)
    -- TODO: Implement this
end

function module.python_project(args)
    -- local_packages should point to tasks that have a "local_requirements" property in their output, containing
    -- a list of requirements to add to requirements.txt files used for building venvs in this project.
    local local_packages = args.local_packages or {}
    local dev_dependencies = args.dev_dependencies or {}

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

    task {
        name = "venv_requirements_file",
        visible = false,
        deps = { tasks = tblext.extend({}, local_packages) },
        artifacts = { files = { requirements = "requirements.venv.txt" } },
        actions = {
            function (c)
                local requirements = ordered_map()
                -- Add all dev dependencies
                for _, dep in pairs(dev_dependencies) do
                    requirements[dep] = true
                end

                -- Add all declared local packages
                for k, task in pairs(c.tasks) do
                    if task.output.local_requirements then
                        for _, req in pairs(task.output.local_requirements) do
                            requirements[req] = true
                        end
                    end
                end
                -- We'll always add our own project in editable mode
                requirements["-e ."] = true

                local f = io.open(path.join(c.project.dir, "requirements.venv.txt"), "w")
                for req, _ in pairs(requirements) do
                    f:write(req.."\n")
                end
                f:close()
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
                c.tool.cmd(tblext.extend({ path.join(".venv", module.venv_bin_path, arg1) }, args))
            end
        }
    }

    task {
        name = "calc_package_source_files",
        visible = false,
        always_run = true,
        deps = {
            files = { pyproject_toml = "pyproject.toml" },
        },
        actions = {
            function (c)
                pyproject = toml.load(c.files.pyproject_toml.path)

                local import_name = maybe(pyproject).tool.flit.module
                    :or_else(function () return pyproject.project.name end)
                    :and_then(function (name) return name:gsub("%.", "/") end)
                    .value
                assert(import_name, "Unable to determine module import name from pyproject.toml")

                local data_dir = maybe(pyproject).tool.flit["external-data"].directory.value
                local includes = maybe(pyproject).tool.flit.sdist.include:or_else(function () return {} end).value
                local excludes = maybe(pyproject).tool.flit.sdist.exclude:or_else(function () return {} end).value

                table.insert(includes, "src/"..import_name.."/**/*")

                if data_dir then
                    table.insert(includes, data_dir.."/**/*")
                end

                table.insert(excludes, "**/*.pyc")

                local package_files = path.glob(c.project.dir, import_name.."/**/*", {include=includes, exclude=excludes, include_dirs=false})

                return { files = package_files }
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
            tasks = tblext.extend({ package_name = "calc_package_sdist_name" }, local_packages),
            calc = { "calc_package_source_files" }
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
                table.insert(local_requirements, string.format("%q", path.join(ws_dir, c.project.dir, "dist", c.tasks.package_name.output)))
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
            tasks = tblext.extend({ package_name = "calc_package_wheel_name" }, local_packages),
            calc = { "calc_package_source_files" }
        },
        artifacts = {
            calc = { "calc_package_wheel" }
        },
        actions = {
            {
                env = { build_venv = build_venv },
                function (c)
                    local tempdir <close> = fs.tempdir()
                    local build_res = c.env.build_venv { "python", "-m", "build", "--wheel", "-o", tempdir.path }
                    local wheel_name = build_res.stdout:match("Successfully built ([^%s]+)")
                    assert(wheel_name)

                    local wheel_path = path.join(tempdir.path, wheel_name)
                    local untagged_wheel_path = path.join(c.project.dir, "dist", c.tasks.package_name.output)

                    fs.mkdir(path.join(c.project.dir, "dist"), { allow_existing = true })
                    fs.rename( wheel_path, untagged_wheel_path )

                    local local_requirements = {}
                    for k, v in pairs(c.tasks) do
                        if type(c.tasks.local_requirements) == "table" then
                            for _, req in pairs(c.tasks.local_requirements) do
                                table.insert(local_requirements, req)
                            end
                        end
                    end
                    table.insert(local_requirements, string.format("%q", untagged_wheel_path))
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
            tasks = tblext.extend({ package_name = "calc_package_wheel_name" }, local_packages),
            calc = { "calc_package_source_files" }
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
                table.insert(local_requirements, "-e "..string.format("%q", path.join(ws_dir, c.project.dir)))
                return {
                    local_requirements = local_requirements,
                    -- Include source file paths and hashes in task output so if they change, tasks that depend on this one will detect the change
                    source_files = c.files
                }
            end
        }
    }

end










return module
