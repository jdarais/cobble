local action, action_context = ...
local tblext = require("tblext")

if type(action[1]) == "function" then
    return pcall(action[1], action_context)
else
    local tool_alias = next(action.tool)
    local env_alias = next(action.env)
    -- Automatically append args if any, if we are a simple cmd-list-style action
    if tool_alias then
        local args = tblext.extend({}, action)
        args.tool = nil
        args.env = nil
        if action_context.args ~= nil then
            tblext.extend(args, action_context.args)
        end
        local success, result = pcall(action_context.tool[tool_alias], args)
        return success, (not success and result or nil)
    elseif env_alias then
        local args = tblext.extend({}, action)
        args.tool = nil
        args.env = nil
        if action_context.args ~= nil then
            tblext.extend(args, action_context.args)
        end
        local success, result = pcall(action_context.env[env_alias], args)
        return success, (not success and result or nil)
    else
        local args = tblext.extend({}, action)
        args.tool = nil
        args.env = nil
        if action_context.args ~= nil then
            tblext.extend(args, action_context.args)
        end
        local success, result = pcall(action_context.tool["cmd"], args)
        return success, (not success and result or nil)
    end
end

