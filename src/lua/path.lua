-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local pathlib = ...

local module_prototype = {
    SEP = pathlib.SEP,
    glob = function(...) return pathlib.glob(...) end,
    is_dir = function(...) return pathlib.is_dir(...) end,
    is_file = function(...) return pathlib.is_file(...) end,
    join = function(...) return pathlib.join(...) end
}

local module_metatable = {
    __index = module_prototype
}

return setmetatable({}, module_metatable)
