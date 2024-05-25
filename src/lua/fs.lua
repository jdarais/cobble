local fslib = ...

local module_prototype = {
    glob = function(...) return fslib.glob(...) end,
    is_dir = function(...) return fslib.is_dir(...) end,
    is_file = function(...) return fslib.is_file(...) end
}

local module_metatable = {
    __index = module_prototype
}

return setmetatable({}, module_metatable)
