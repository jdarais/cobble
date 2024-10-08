local path = require("path")
local iter = require("iter")
local tblext = require("tblext")
local script_dir = require("script_dir")

local venv_python = PLATFORM.os_family == "windows"
    and path.join(".venv", "Scripts", "python.exe")
    or path.join(".venv", "bin", "python")

env {
    name = "venv",
    setup_task = {
        actions = { { tool = "python", "-m", "venv", ".venv" } },
        artifacts = { files = { ".venv/pyvenv.cfg" } }
    },
    action = { tool = "cmd", path.join(script_dir(), venv_python), "-m" }
}

env {
    name = "mkdocs_env",
    setup_task = {
        actions = { { env = "venv", "pip", "install", "-c", "constraints.txt", "mkdocs-material" } },
        deps = {
            files = { "constraints.txt" }
        },
        artifacts = {
            files = {
                (PLATFORM.os_family == "windows"
                    and path.join(".venv", "Scripts", "mkdocs.exe")
                    or path.join(".venv", "bin", "mkdocs"))
            }
        }
    },
    action = { env = "venv", "mkdocs" }
}

task {
    name = "calc_docs_src_files",
    always_run = true,
    actions = {
        function (c)
            local src_files = iter(ipairs(path.glob(path.join(c.project.dir), "cobble/**/*")))
                :filter(function(i, f) return path.is_file(path.join(c.project.dir, f)) end)
                :to_table()
            return { files = src_files }
        end
    }
}

task {
    name = "build",
    actions = {
        {
            env = "mkdocs_env",
            function (c)
                c.env.mkdocs_env { cwd = "cobble", "build" }
            end
        }
    },
    deps = { calc = { "calc_docs_src_files" } }
}
