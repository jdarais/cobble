local cxt = ...

local _require = require
require = function (modname)
    mod, fname = _require(modname)
    if fname and fname ~= ":preload:" then
        table.insert(PROJECT.project_source_deps, cxt.strip_path_prefix(fname, cxt.ws_dir))
    end
    return mod, fname
end

local path = require("path")

cobble = {
    projects = {},
}

PROJECT = nil

_project_stack = {}

function start_project (name, dir)
    local project_source_deps = {}
    if PROJECT then
        if name == "" then
            error("Empty name is only allowed for the root project!")
        end

        if PROJECT.name == "/" then
            name = "/" .. name
        else
            name = PROJECT.name .. "/" .. name
        end

        if dir then
            project_source_deps = { path.join(dir, cxt.project_file_name) }
        else
            project_source_deps = { table.unpack(PROJECT.project_source_deps) }
        end
        dir = dir or PROJECT.dir
    else
        name = "/" .. (name or "")
        if dir then
            project_source_deps = { path.join(dir, cxt.project_file_name) }
        end
        dir = dir or WORKSPACE.dir
    end

    if cobble.projects[name] then
        error("Project " .. name .. " already exists!")
    end

    local project = {
        name = name,
        dir = dir,
        build_envs = {},
        tasks = {},
        tools = {},
        child_projects = {},
        project_source_deps = project_source_deps
    }
    
    if PROJECT then table.insert(PROJECT.child_projects, project) end

    cobble.projects[name] = project
    table.insert(_project_stack, project)
    PROJECT = project
end

function end_project ()
    table.remove(_project_stack)
    PROJECT = _project_stack[#_project_stack]
end

function project (proj)
    start_project(proj.name)
    proj.def()
    end_project()
end

function project_dir (dir)
    cxt.process_project_dir(dir)
end

function env (en)
    local status, err = pcall(cxt.validate_build_env, en)
    if not status then error(err, 1) end
    table.insert(PROJECT.build_envs, en)
end

function tool (tl)
    cxt.validate_tool(tl)
    table.insert(PROJECT.tools, tl)
end

function task (tsk)
    local status, err = pcall(cxt.validate_task, tsk)
    if not status then error(err, 1) end
    table.insert(PROJECT.tasks, tsk)
end
