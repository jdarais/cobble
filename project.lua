local path = require("path")

require("tools")

local example_test_dirs = path.glob("examples/*/tests")

for i, d in ipairs(example_test_dirs) do
    if path.is_file(path.join(d, "project.lua")) then
        project_dir(d)
    end
end

task {
    name = "find_cobble_source_files",
    always_run = true,
    actions = { function (c) return { files = path.glob("src/**/*.*") } end }
}

task {
    name = "calc_build_dep",
    actions = { function (c)
        return { tasks = { (c.vars["cobble.build"] == "release" and "build_release") or "build_debug" } }
    end },
    deps = { vars = { "cobble.build" } }
}

task {
    name = "build_release",
    actions = { { tool = "cargo", "build", "--release" } },
    deps = { calc = { "find_cobble_source_files" } },
    artifacts = { "target/release/cobl" .. (PLATFORM.os_family == "windows" and ".exe" or "") }
}

task {
    name = "build_debug",
    actions = { { tool = "cargo", "build" } },
    deps = { calc = { "find_cobble_source_files" } },
    artifacts = { "target/debug/cobl" .. (PLATFORM.os_family == "windows" and ".exe" or "") }
}

task {
    name = "build",
    actions = {},
    deps = { calc = { "calc_build_dep" } }
}

