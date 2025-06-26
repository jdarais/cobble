-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local function extend(target, source, start_index_or_opts)
    if source == nil then
        return target
    end

    if type(target) ~= "table" then
        error("Expected a table for first argument to extend(), but got "..tostring(target))
    end

    if type(source) ~= "table" then
        error("Expected a table for second argument to extend(), but got "..tostring(source))
    end

    local valid_start_index_types = { ["number"] = true, ["nil"] = true }
    if not valid_start_index_types[type(start_index)] then
        error("Expected a number or nil for third argument to extend(), but got "..tostring(start_index))
    end

    local opts
    if type(start_index_or_opts) == "table" then
        opts = start_index_or_opts
    else 
        opts = { start_index = start_index_or_opts }
    end

    local start_offset = (opts.start_index or (#target+1)) - 1

    for k, v in pairs(source) do
        if type(k) == "number" then
            local existing = target[k+start_offset]
            if opts.deep and type(existing) == "table" and type(v) == "table" then
                extend(existing, v, opts)
            else
                target[k+start_offset] = v
            end
        else
            local existing = target[k]
            if opts.deep and type(existing) == "table" and type(v) == "table" then
                extend(existing, v, opts)
            else
                target[k] = v
            end
        end
    end

    return target
end

local function format(tbl)
    if type(tbl) == "string" then
        return "\"" .. tbl .. "\""
    elseif type(tbl) == "table" then
        local tbl_str = "{"
        for k, v in pairs(tbl) do
            tbl_str = tbl_str .. "[" .. format(k) .. "]=" .. format(v) .. ", "
        end
        tbl_str = tbl_str .. "}"
    
        return tbl_str
    else
        return tostring(tbl)
    end
end

local tblext_module_prototype = {
    extend = extend,
    format = format
}

local tblext_module_metatable = {
    __index = tblext_module_prototype
}

return setmetatable({}, tblext_module_metatable)
