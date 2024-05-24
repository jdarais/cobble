require("tools")

project_dir("test")

task {
    name = "find_cobble_source_files",
    actions = { function (c) return { files = fs.glob("src/**/*.*") } end }
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
    artifacts = { "target/release/cobl" }
}

task {
    name = "build_debug",
    actions = { { tool = "cargo", "build" } },
    deps = { calc = { "find_cobble_source_files" } },
    artifacts = { "target/debug/cobl" }
}

task {
    name = "build",
    actions = {},
    deps = { calc = { "calc_build_dep" } }
}

