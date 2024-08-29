local path = require("path")
local iter = require("iter")
local tblext = require("tblext")

task {
    name = "calc_node_typescript_test_repo_files",
    always_run = true,
    actions = { function (c)
        local deps = {
            files = iter(ipairs(path.glob(c.project.dir, path.join("../workspace/**/*"))))
                :filter(function (i, f) return not f:match("node_modules[/\\]") end)
                :filter(function (i, f) return not f:match("lib[/\\]") end)
                :filter(function (i, f) return path.is_file(path.join(c.project.dir, f)) end)
                :to_table()
        }
        return deps
    end }
}

task {
    name = "node_typescript_image",
    actions = { { tool = "docker", "build", "-f", "node_typescript.Dockerfile", "-t", "local/cobble_test_node_typescript", "../../.." } },
    deps = {
        files = { "node_typescript.Dockerfile", "../../../.dockerignore", "../../../target/release/cobl" },
        calc = { "calc_node_typescript_test_repo_files" }
    }
}

task {
    name = "test_npm_tool_check",
    default = true,
    actions = { { tool = "docker", "run", "--rm", "local/cobble_test_node_typescript", "cobl", "tool", "check", "npm" } },
    deps = { tasks = { "node_typescript_image" } }
}

task {
    name = "test_build",
    default = true,
    actions = { { tool = "docker", "run", "--rm", "local/cobble_test_node_typescript", "cobl", "run", "packages/pkg1/build" } },
    deps = { tasks = { "node_typescript_image" } }
}


