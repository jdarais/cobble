-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)]
--
-- An order-preserving map implementation for Lua

local array_metatable = ...

--
-- array - An array container that only allows contiguous, numeric indices.  When serialized, arrays are always
-- serialized as lists, not maps or tables.  This helps to ambiguate the intent at serialization time, since
-- Lua tables serve as both maps and lists, but most serialization formats differentiate between the two.
--

local array_methods = {
    push = function(self, value)
        table.insert(self, value)
    end,

    append = function(self, other)
        for i, v in ipairs(other) do
            table.insert(self, v)
        end
    end,

    map = function(self, map_fn)
        local result = setmetatable({}, array_metatable)
        for i, v in ipairs(self) do
            -- index and value going into map_fn are intentionally reversed.  This allows simple
            -- map functions that just operate on the value, but the index is available if needed
            result:push(map_fn(v, i))
        end
        return result
    end
}

array_metatable.__index = array_methods
function array_metatable.__newindex(self, key, value)
    if type(key) ~= "number" then
        error("Only number keys are allowed in arrays")
    end

    if key < 1 or key > #self+1 then
        error("Array index out of bounds: "..key)
    end

    rawset(self, key, value)
end

function array_metatable.__add(self, other)
    local result = setmetatable({}, array_metatable)
    result:append(self)
    result:append(other)
    return result
end

function array_metatable.__tostring(self, visited)
    if visited == nil then
        visited = {}
    end
    
    if visited[self] then
        return "..."
    end

    visited[self] = true

    local s = "{ "
    for i, v in ipairs(self) do
        s = s..tostring(i, visited).." = "..tostring(v, visited)..", "
    end
    s = s.."}"
    return s
end


local function array(init)
    local result = setmetatable({}, array_metatable)
    if init ~= nil then
        result:append(init)
    end
    return result
end

--
-- map - A map type that only accepts string keys.  The map type also provides additional functions that make it
-- easy to work with map data
--

local map_methods = {
    update = function(self, other)
        for k, v in pairs(other) do
            self[k] = v
        end
    end,

    remove = function(self, key)
        local removed = self[key]
        self[key] = nil
        return removed
    end,

    keys = function(self)
        local result = array()
        for k, v in pairs(self) do
            result:push(k)
        end
        return result
    end,

    values = function(self)
        local result = array()
        for k, v in pairs(self) do
            result:push(v)
        end
        return result
    end
}

local map_metatable = {
    __index = map_methods,
    __newindex = function(self, key, value)
        if type(key) ~= "string" then
            error("Only string keys are allowed in maps")
        end

        rawset(self, key, value)
    end,
    __tostring = function(self, visited)
        if visited == nil then
            visited = {}
        end
        
        if visited[self] then
            return "..."
        end

        visited[self] = true
        
        local s = "{ "
        for k, v in pairs(self) do
            s = s..tostring(k, visited).." = "..tostring(v, visited)..", "
        end
        s = s.."}"
        return s
    end
}

local function map(init)
    local result = setmetatable({}, map_metatable)
    if init ~= nil then
        result:update(init)
    end
    return result
end

local function ordered_map_pairs_next(state, k)
    state.index = state.index + 1
    local next_key = state.map._keys[state.index]
    if next_key == nil then
        return nil
    end

    local next_val = state.map._map[next_key]
    return next_key, next_val
end

--
-- ordered_map - a map that preserves the order of entries. Entries are kept in the order they were added in.
--

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

return {
    map = map,
    array = array,
    ordered_map = ordered_map
}
