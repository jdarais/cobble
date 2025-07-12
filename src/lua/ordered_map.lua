-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)]
--
-- An order-preserving map implementation for Lua

local function ordered_map_pairs_next(state, k)
    state.index = state.index + 1
    local next_key = state.map._keys[state.index]
    if next_key == nil then
        return nil
    end

    local next_val = state.map._map[next_key]
    return next_key, next_val
end

local ordered_map_metatable = {
    __index = function (self, key)
        return self._map[key]
    end,
    __newindex = function (self, key, value)
        local existing = self._map[key]
        if value == nil then
            if existing ~= nil then
                -- Find the index of the key in the list of keys
                local key_index = false
                for i, k in ipairs(self._keys) do
                    if k == key then
                        key_index = i
                        break
                    end
                end
                -- Shift keys left, writing over the deleted key
                local keys_len = #self._keys
                for i = key_index+1, keys_len do
                    self._keys[i-1] = self._keys[i]
                    self._keys[i] = nil
                end
                -- Delete value at key from the map
                self._map[key] = nil
            end
        else -- value ~= nil
            if existing == nil then
                local index = #self._keys + 1
                self._keys[index] = key
            end
            self._map[key] = value
        end
    end,
    __pairs = function (self)
        return ordered_map_pairs_next, { map=self, index=0 }, ""
    end,
    __len = function (self)
        return #self._keys
    end
}

local function ordered_map(entries)
    local map = setmetatable({
        _map = {},
        _keys = {}
    }, ordered_map_metatable)

    if entries then
        for i, entry in ipairs(entries) do
            map[entry[1]] = entry[2]
        end
    end

    return map
end

return ordered_map
