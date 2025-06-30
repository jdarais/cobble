-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local fslib = ...

local module_prototype = {
    mkdir = function(...) return fslib.mkdir(...) end,
    rmdir = function(...) return fslib.rmdir(...) end,
    copy = function(...) return fslib.copy(...) end,
    remove = function(...) return fslib.remove(...) end,
}

local module_metatable = {
    __index = module_prototype
}

return setmetatable({}, module_metatable)
