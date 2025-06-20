local path = require("path")
local iter = require("iter")
local tblext = require("tblext")

task {
    name = "calc_node_typescript_test_repo_files",
    always_run = true,
    actions = { function (c)
        local deps = {
            files = path.glob(
                c.project.dir,
                "../workspace/**/*",
                {
                    exclude = {"../workspace/node_modules/**", "lib/**" },
                    include_dirs = false
                }
            )
        }
        return deps
    end }
}

task {
    name = "node_typescript_image",
    actions = {
        function (c) c.println("Using dockerfile: "..c.files.dockerfile.path) end,
        { tool = "docker", "build", "-f", "node_typescript.Dockerfile", "-t", "local/cobble_test_node_typescript", "../../.." }
    },
    deps = {
        files = { dockerfile = "node_typescript.Dockerfile", "../../../.dockerignore", "../../../target/release/cobl" },
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


