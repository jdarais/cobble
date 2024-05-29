local path = require("path")
local maybe = require("maybe")
local iter = require("iter")
local toml = require("toml")
local tblext = require("tblext")
local cmd = require("cmd")
local version = require("version")

tool {
    name = "poetry",
    check = function (c)
        local res = cmd { "poetry", "--version" }
        assert(res.status == 0, "poetry command exited with status " .. res.status)

        local poetry_version = res.stdout:match("Poetry %(version (%S+)%)")
        assert(version(poetry_version) >= "1.8.0", "Poetry >= 1.8.0 required. Found ".. poetry_version)
    end,
    action = { tool = "cmd", "poetry" }
}

env {
    name = "poetry_env",
    install = {
        { tool = "poetry", function (c)
            c.tool.poetry { "env", "use", c.vars["python.version"] }
        end },
        { tool = "poetry", "install" }
    },
    deps = {
        files = { "poetry.lock" },
        vars = { "python.version" }
    },
    action = { tool = "poetry", "run" }
}

task {
    name = "poetry_lock",
    deps = { files = { "pyproject.toml" } },
    artifacts = { "poetry.lock" },
    actions = { { tool = "poetry", "lock", "--no-update" } },
}

task {
    name = "shell",
    always_run = true,
    interactive = true,
    actions = { { env = "poetry_env", "python" } }
}

task {
    name = "lint",
    actions = { { env = "poetry_env", "python", "-m", "pylint", "python_poetry/" } },
    deps = { calc = { "find_poetry_source_files" } }
}

task {
    name = "typecheck",
    actions = { { env = "poetry_env", "python", "-m", "mypy", "-p", "python_poetry" } },
    deps = { calc = { "find_poetry_source_files" } }
}

task {
    name = "format.check",
    actions = { { env = "poetry_env", "python", "-m", "black", "--check", "." } },
    deps = { calc = { "find_poetry_source_files" } }
}

task {
    name = "format",
    always_run = true,
    actions = { { env = "poetry_env", "python", "-m", "black", "." } }
}

task {
    name = "find_poetry_source_files",
    deps = { files = { "pyproject.toml" } },
    actions = {
        function (c)
            local pyproject_toml = toml.load("pyproject.toml")
            local patterns = maybe(pyproject_toml)["tool"]["poetry"]["packages"]
                :or_else(function ()
                    return maybe(pyproject_toml)["tool"]["poetry"]["name"]
                        :and_then(function (name) return { { include = name:gsub("-", "_") }, } end)
                        :or_else(function () error("Expected name to exist in pyproject.toml") end)
                        .value
                end)
                .value

            local include_files = {}
            local exclude_files = {}
            for i, v in ipairs(patterns) do
                if v.include then
                    local pattern = path.is_dir(v.include) and (v.include .. "/**/*") or v.include
                    iter(ipairs(path.glob(pattern)))
                        :filter(function(_, f) return not f:match("%.pyc$") end)
                        :filter(function(_, f) return not f:match("[/\\]__pycache__[/\\]") end)
                        :for_each(function(_, f) include_files[f] = f end)
                end
                if v.exclude then
                    local pattern = path.is_dir(v.exclude) and (v.exclude .. "/**/*") or v.exclude
                    for i, f in ipairs(path.glob(pattern)) do
                        exclude_files[f] = true
                    end 
                end
            end
            for f, _ in pairs(exclude_files) do
                include_files[f] = nil
            end
            return { files = include_files }
        end
    }
}

