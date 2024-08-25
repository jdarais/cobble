-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local action, action_context, return_arg_list_action_result = ...
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
        if success and not return_arg_list_action_result then
            return success, nil
        else
            return success, result
        end
    elseif env_alias then
        local args = tblext.extend({}, action)
        args.tool = nil
        args.env = nil
        if action_context.args ~= nil then
            tblext.extend(args, action_context.args)
        end
        local success, result = pcall(action_context.env[env_alias], args)
        if success and not return_arg_list_action_result then
            return success, nil
        else
            return success, result
        end
    else
        local args = tblext.extend({}, action)
        args.tool = nil
        args.env = nil
        if action_context.args ~= nil then
            tblext.extend(args, action_context.args)
        end
        local success, result = pcall(action_context.tool["cmd"], args)
        if success and not return_arg_list_action_result then
            return success, nil
        else
            return success, result
        end
    end
end

