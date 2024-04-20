
local exports = {}


function exports.poetry_project ()
    build_env({
        name = "poetry",
        install_actions = {
            { tool = "/poetry", "lock" },
            { tool = "/poetry", "install" }
        },
        deps = {
            files = { "pyproject.toml", "poetry.lock" }
        },
        action = {
            tool = "poetry",
            function(cxt)
                return cxt.tool.poetry { cwd = cxt.cwd, "run", table.unpack(cxt.args) }
            end
        }
    })

    task({
        name = "calc_poetry_build_deps",
        hidden = true,
        actions = {
            build_env = "poetry",
            exec = function (self, cxt, args)
                local deps = {}
                local res = cxt:exec { "python", WORKSPACE.dir .. "/cobble_plugins/poetry_build_deps.py" } ;
                for dep in res.stdout.gmatch("([^\r\n]*)") do
                    table.insert(deps, dep)
                end
                return { files = deps }
            end
        },
        artifacts = {
            files = { ".poetry_build_deps" }
        }
    })

    task({
        name = "build",
        build_env = "poetry",
        actions = {
            { "build" }
        },
        deps = {
            calc = { "calc_poetry_build_deps" }
        }
    })
end

return exports



