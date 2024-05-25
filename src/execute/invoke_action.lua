local action, action_context = ...

if type(action[1]) == "function" then
    return pcall(action[1], action_context)
elseif type(action[1]) == "userdata" then
    return pcall(action[1].invoke, action[1], action_context)
else
    local tool_alias = next(action.tool)
    local env_alias = next(action.env)
    -- Automatically append args if any, if we are a simple cmd-list-style action
    if tool_alias then
        local args = {table.unpack(action)}
        if action_context.args ~= nil then
            table.move(action_context.args, 1, #action_context.args, #args+1, args)
        end
        local success, result = pcall(action_context.tool[tool_alias], args)
        return success, (not success and result or nil)
    elseif env_alias then
        local args = {table.unpack(action)}
        if action_context.args ~= nil then
            table.move(action_context.args, 1, #action_context.args, #args+1, args)
        end
        local success, result = pcall(action_context.env[env_alias], args)
        return success, (not success and result or nil)
    else
        local args = {table.unpack(action)}
        if action_context.args ~= nil then
            table.move(action_context.args, 1, #action_context.args, #args+1, args)
        end
        local success, result = pcall(action_context.tool["cmd"], args)
        return success, (not success and result or nil)
    end
end

