local npm = require("cobble_modules.npm")

npm.npm_package()

task {
    name = "build",
    actions = { { tool = "cmd", "echo", "building..." } },
    deps = {
        calc = { "calc_package_dep_build_tasks" }
    }
}
