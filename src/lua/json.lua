local json_lib = ...

local json_module_prototype = {
    loads = function (...) return json_lib.loads(...) end,
    load = function (...) return json_lib.load(...) end,
    dumps = function (...) return json_lib.dumps(...) end,
    dump = function (...) return json_lib.dump(...) end,
}

local json_module_metatable = {
    __index = json_module_prototype
}

return setmetatable({}, json_module_metatable)
