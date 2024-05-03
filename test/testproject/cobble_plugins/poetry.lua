local exports = {}

function exports.poetry_project ()
    build_env({
        name = "poetry",
        install = {
            { tool = "poetry", "lock" },
            { tool = "poetry", "install" }
        },
        deps = {
            files = { "pyproject.toml", "poetry.lock" }
        },
        action = {
            tool = "poetry",
            function(c)
                return c.tool.poetry { "run", table.unpack(c.args) }
            end
        }
    })

    task({
        name = "calc_poetry_build_deps",
        hidden = true,
        actions = {
            {
                build_env = "poetry",
                function (c)
                    local deps = {}
                    local res = c.env.poetry { "python", script_dir() .. "/poetry_build_deps.py" } ;
                    for dep in res.stdout:gmatch("([^\r\n]+)") do
                        table.insert(deps, dep)
                    end
                    return { files = deps }
                end
            }
        },
        artifacts = {
            files = { ".poetry_build_deps" }
        }
    })

    task({
        name = "build",
        actions = {
            { tool = "poetry", "build" }
        },
        deps = {
            calc = { "calc_poetry_build_deps" }
        }
    })
end

return exports



