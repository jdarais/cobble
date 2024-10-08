local path = require("path")
local toml = require("toml")
local tblext = require("tblext")

require("tools")

project_dir("docs")

-- Example projects
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

if PLATFORM.os_family == "windows" then
task {
    name = "build_release_linux",
    actions = { { tool = "wsl", "--shell-type", "login", "--", "cargo", "build", "--release" } },
    deps = { calc = { "find_cobble_source_files" } },
    artifacts = { "target/release/cobl" }
}
end

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

--------- Tasks for managing release automation -----------

local noop = function () end

task {
    name = "version_tag",
    output = "always",
    always_run = true,
    actions = {
        {
            tool = "git",
            function (c)
                local cargo_toml = toml.load("Cargo.toml")
                local version_tag = "v" .. cargo_toml["package"]["version"]
                c.println("Verison tag from Cargo.toml: " .. version_tag)
                c.println("Getting latest version tag...")
                local version_tag_exists, cmd_result = pcall(c.tool.git, { out=noop, err=noop, "rev-parse", version_tag })
                if not version_tag_exists then
                    c.println("Creating and pushing tag: " .. version_tag)
                    c.tool.git { "tag", version_tag }
                    c.tool.git { "push", "origin", version_tag }
                else
                    c.println("Tag " .. version_tag .. " already exists.")
                end
            end
        }
    }
}

