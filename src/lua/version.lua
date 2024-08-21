-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local VER_SUBCOMPONENT_PATTERN = "(%w*)([^%w]*)"

local iter = require("iter")

local function version_cmp(v1, v2)
    local v1_str = tostring(v1)
    local v2_str = tostring(v2)

    local v1_components = iter(v1_str:gmatch("[^.]+")):enumerate():to_table()
    local v2_components = iter(v2_str:gmatch("[^.]+")):enumerate():to_table()

    for i = 1, math.max(#v1_components, #v2_components) do
        local c1 = v1_components[i]
        local c2 = v2_components[i]

        if not c1 then return -1 end
        if not c2 then return 1 end

        local c1_subs = iter(c1:gmatch(VER_SUBCOMPONENT_PATTERN))
            :reduce({}, function(tbl, sub, sep)
                table.insert(tbl, sub)
                table.insert(tbl, sep)
                return tbl
            end)

        local c2_subs = iter(c2:gmatch(VER_SUBCOMPONENT_PATTERN))
            :reduce({}, function(tbl, sub, sep)
                table.insert(tbl, sub)
                table.insert(tbl, sep)
                return tbl
            end)

        for j = 1, math.max(#c1_subs, #c2_subs) do
            local s1 = c1_subs[j]
            local s2 = c2_subs[j]

            -- Difference between subcomponents and "."-separated components:
            -- Extra subcomponents are considered prerelease markers, so if one list
            -- of subcomponents runs out first, it will be considered greater than the
            -- other list
            local s1_empty = not s1 or s1 == ""
            local s2_empty = not s2 or s2 == ""
            if s1_empty and not s2_empty then return 1 end
            if s2_empty and not s1_empty then return -1 end

            local s1_num = tonumber(s1)
            local s2_num = tonumber(s2)

            local res = 0
            if      s1_num and s2_num   then res = s1_num - s2_num
            elseif  s1_num              then res = 1
            elseif  s2_num              then res = -1
            elseif  s1 < s2             then res = -1
            elseif  s1 > s2             then res = 1
            end

            if res ~= 0 then return res end
        end
    end

    return 0
end

local version_metatable = {
    __eq = function(lhs, rhs) return version_cmp(lhs, rhs) == 0 end,
    __lt = function(lhs, rhs) return version_cmp(lhs, rhs) < 0 end,
    __le = function(lhs, rhs) return version_cmp(lhs, rhs) <= 0 end,
    __tostring = function(v) return v.ver end
}

local function version(v)
    return setmetatable({
        ver = tostring(v)
    }, version_metatable)
end

local version_module_prototype = {
    cmp = version_cmp
}

local version_module_metatable = {
    __index = version_module_prototype,
    __call = function(mod, ...) return version(...) end
}

return setmetatable({}, version_module_metatable)
