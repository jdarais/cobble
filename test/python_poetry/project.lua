

external_tool {
    name = "poetry",
    action = { tool = "cmd", "poetry" }
}

task {
    name = "poetry_lock",
    deps = { files = { "pyproject.toml" } },
    artifacts = { "poetry.lock" },
    actions = {
        { tool = "poetry", "lock" }
    },
}

task {
    name = "find_poetry_source_files",
    deps = { files = { "pyproject.toml" } },
    actions = {
        function (c)
            local pyproject_toml = toml.read("pyproject.toml")
            local patterns = maybe(pyproject_toml)["tool"]["poetry"]["packages"]
                :or_else(function ()
                    return maybe(pyproject_toml)["tool"]["poetry"]["name"]
                        :and_then(function (name) return { { include = name:gsub("-", "_") }, } end)
                        :or_else(function () error("Expected name to exist in pyproject.toml") end)
                        .value
                end)
                .value
            local include_files_set = {}
            local exclude_files_set = {}
            for i, v in ipairs(patterns) do
                if v.include then
                    local pattern = if_else(fs.is_dir(v.include), v.include .. "/**/*", v.include)
                    local files = iter(ipairs(fs.glob(pattern)))
                        :filter(function(_, f) return not f:match("%.pyc$") end)
                        :filter(function(_, f) return not f:match("[/\\]__pycache__[/\\]") end)
                    for i, f in files:iterator() do
                        include_files_set[f] = true
                    end
                end
                -- TODO: v.exclude
            end
            local include_files = {}
            for k, v in pairs(include_files_set) do
                if not exclude_files_set[k] then
                    table.insert(include_files, k)
                end
            end
            return include_files
        end
    }
}

build_env {
    name = "poetry_env",
    init = {
        { tool = "poetry", "install" }
    },
    deps = {
        files = { "poetry.lock" }
    },
    action = { tool = "poetry", "run" }
}

task {
    name = "lint",
    actions = {
        { env = "poetry_env", "python", "-m", "pylint", "python_poetry/" }
    }
}


