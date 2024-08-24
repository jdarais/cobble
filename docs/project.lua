local path = require("path")
local iter = require("iter")
local tblext = require("tblext")

env {
    name = "poetry_env",
    setup_task = {
        actions = { { tool = "poetry", "install" } },
        deps = {
            files = { "poetry.lock" }
        }
    },
    action = { tool = "poetry", "run" }
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
            env = "poetry_env",
            function (c)
                c.env.poetry_env { cwd = path.join(c.project.dir, "cobble"), "mkdocs", "build" }
            end
        }
    },
    deps = { calc = { "calc_docs_src_files" } }
}
