local tblext = require("tblext")
local path = require("path")
local python = require("python")

project_dir("pkg1")
project_dir("pkg2")

local ws_dir = WORKSPACE.dir

tool {
    name = "python",
    action = { "python" }
}

task {
    name = "pip_tools_constraints_file",
    visible = false,
    actions = {
        {
            tool = "python",
            function (c)
                local sys_platform_res = c.tool.python { "-c", "import sys; print(sys.platform)" }
                return {
                    files = {
                        pip_tools_constraints_file = "constraints.pip-tools."..sys_platform_res.stdout:match("[^%s]*")..".txt"
                    }
                }
            end
        }
    }
}

task {
    name = "constraints_file_name",
    visible = false,
    actions = {
        { tool = "python", function (c)
            local sys_platform_res = c.tool.python { "-c", "import sys; print(sys.platform)" }
            return path.join(ws_dir, "constraints."..sys_platform_res.stdout:match("[^%s]*")..".txt")
        end }
    }
}

task {
    name = "constraints_file_dep",
    visible = false,
    deps = { tasks = { constraints_file_name = "constraints_file_name" } },
    actions = { function (c)
        return { files = { constraints_file = c.tasks.constraints_file_name.output } }
    end }
}

env {
    name = "pip_tools_venv",
    setup_task = {
        deps = {
            calc = { "pip_tools_constraints_file" }
        },
        actions = {
            { tool = "python", "-m", "venv", ".venv-pip-tools" },
            function (c)
                c.tool.cmd {
                    path.join(".venv-pip-tools", python.venv_python_path),
                    "-m", "pip", "install",
                    "-c", path.strip_prefix(c.files.pip_tools_constraints_file.path, c.project.dir),
                    "pip-tools", "build"
                }
            end
        }
    },
    action = function (c)
        local args = {table.unpack(c.args)}
        local arg1 = table.remove(args, 1)
        return c.tool.cmd(tblext.extend({path.join(ws_dir, ".venv-pip-tools", python.venv_bin_path, arg1)}, args))
    end
}

task {
    name = "update_pip_tools_constraints",
    description = "Update the pinned constraints file used to install the pip tools venv",
    deps = { tasks = { constraints_file = "pip_tools_constraints_file" } },
    actions = {
        {
            env = "pip_tools_venv",
            function (c)
                c.env.pip_tools_venv {
                    out = false, err = false,
                    "python", "-m", "piptools", "compile",
                    "--strip-extras",
                    "-o", path.strip_prefix(c.tasks.constraints_file.output.files.pip_tools_constraints_file, c.project.dir),
                    "requirements.pip-tools.in"
                }
            end
        }
    }
}

task {
    name = "pin_constraints",
    description = "Generate a pinned constraints file based on requirements.in",
    deps = {
        files = { "requirements.in" },
        tasks = { constraints_file = "constraints_file_name" }
    },
    artifacts = {
        calc = { "constraints_file_dep" }
    },
    actions = {
        {
            env = "pip_tools_venv",
            function (c)
                c.env.pip_tools_venv {
                    out = false, err = false,
                    "python", "-m", "piptools", "compile",
                    "--strip-extras",
                    "-o", path.strip_prefix(c.tasks.constraints_file.output, path.join(ws_dir, c.project.dir)),
                    "requirements.in"
                }
            end
        }
    }
}
