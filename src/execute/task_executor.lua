-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

cobble = {
    _tool_cache = {},
    _build_env_cache = {}
}

local create_action_context, invoke_tool, invoke_build_env, invoke_action

create_action_context = function (
    action,
    extra_tools,
    extra_build_envs,
    file_hashes,
    vars,
    task_outputs,
    project_dir,
    out,
    err,
    args
)
    local action_context = {
        tool = {},
        env = {},
        files = file_hashes,
        vars = vars,
        tasks = task_outputs,
        args = args,
        action = action,
        project = { dir = project_dir },
        out = out,
        err = err
    }

    for tool_alias, tool_name in pairs(extra_tools) do
        action_context.tool[tool_alias] = function (tool_args)
            return cobble.invoke_tool(tool_name, project_dir, out, err, tool_args)
        end
    end
    for tool_alias, tool_name in pairs(action.tool) do
        action_context.tool[tool_alias] = function (tool_args)
            return cobble.invoke_tool(tool_name, project_dir, out, err, tool_args)
        end
    end

    for env_alias, env_name in pairs(extra_build_envs) do
        local env_task_outputs = { install = task_outputs[env_alias] }
        action_context.env[env_alias] = function (env_args)
            return cobble.invoke_build_env(env_name, env_task_outputs, project_dir, out, err, env_args)
        end
    end
    for env_alias, env_name in pairs(action.env) do
        local env_task_outputs = { install = task_outputs[env_alias] }
        action_context.env[env_alias] = function (env_args)
            return cobble.invoke_build_env(env_name, env_task_outputs, project_dir, out, err, env_args)
        end
    end

    return action_context
end

invoke_action = function(action, action_context)
    if type(action[1]) == "function" then
        return action[1](action_context)
    elseif type(action[1]) == "userdata" then
        return action[1]:invoke(action_context)
    else
        local tool_alias = next(action.tool)
        local env_alias = next(action.env)
        -- Automatically append args if any, if we are a simple cmd-list-style action
        if tool_alias then
            local args = {table.unpack(action)}
            if action_context.args ~= nil then
                table.move(action_context.args, 1, #action_context.args, #args+1, args)
            end
            action_context.tool[tool_alias](args)
        elseif env_alias then
            local args = {table.unpack(action)}
            if action_context.args ~= nil then
                table.move(action_context.args, 1, #action_context.args, #args+1, args)
            end
            action_context.env[env_alias](args)
        else
            local args = {table.unpack(action)}
            if action_context.args ~= nil then
                table.move(action_context.args, 1, #action_context.args, #args+1, args)
            end
            action_context.tool["cmd"](args)
        end
    end   
end

invoke_tool = function (name, project_dir, out, err, args)
    local action = cobble._tool_cache[name].action
    local action_context = create_action_context(action, {}, {}, {}, {}, {}, project_dir, out, err, args)
    return invoke_action(action, action_context)
end

invoke_build_env = function (name, task_outputs, project_dir, out, err, args)
    local action = cobble._build_env_cache[name].action
    local action_context = create_action_context(action, {}, {}, {}, {}, task_outputs, project_dir, out, err, args)
    return invoke_action(action, action_context)
end

cobble.invoke_tool = invoke_tool
cobble.invoke_build_env = invoke_build_env
cobble.create_action_context = create_action_context
cobble.invoke_action = invoke_action
