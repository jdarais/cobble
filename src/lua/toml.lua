local toml_lib = ...

local toml_module_prototype = {
    loads = function (...) return toml_lib.loads(...) end,
    load = function (...) return toml_lib.load(...) end,
    dumps = function (...) return toml_lib.dumps(...) end,
    dump = function (...) return toml_lib.dump(...) end,
}

local toml_module_metatable = {
    __index = toml_module_prototype
}

return setmetatable({}, toml_module_metatable)
