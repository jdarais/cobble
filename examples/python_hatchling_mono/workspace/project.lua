local tblext = require("tblext")
local path = require("path")

project_dir("pkg1")

local ws_dir = WORKSPACE.dir

local python_exe, venv_python_path
if PLATFORM.os == "windows" then
    python_exe = "python.exe"
    venv_python_path = path.join("Scripts", "python.exe")
else
    python_exe = "python3"
    venv_python_path = path.join("bin", "python")
end

tool {
    name = "python",
    action = { "python" }
}

task {
    name = "pip_tools_constraints_file",
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
    actions = {
        { tool = "python", function (c)
            local sys_platform_res = c.tool.python { "-c", "import sys; print(sys.platform)" }
            return path.join(ws_dir, "constraints."..sys_platform_res.stdout:match("[^%s]*")..".txt")
        end }
    }
}

task {
    name = "constraints_file_dep",
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
                    path.join(".venv-pip-tools", venv_python_path),
                    "-m", "pip", "install",
                    "-c", path.strip_prefix(c.files.pip_tools_constraints_file.path, c.project.dir),
                    "pip-tools"
                }
            end
        }
    },
    action = { path.join(".venv-pip-tools", venv_python_path) }
}

task {
    name = "update_pip_tools_constraints",
    deps = { tasks = { constraints_file = "pip_tools_constraints_file" } },
    actions = {
        { 
            env = "pip_tools_venv",
            function (c)
                c.env.pip_tools_venv {
                    out = false, err = false,
                    "-m", "piptools", "compile",
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
                    "-m", "piptools", "compile",
                    "--strip-extras",
                    "-o", path.strip_prefix(c.tasks.constraints_file_name.output, c.project.dir),
                    "requirements.in"
                }
            end
        }
    }
}
