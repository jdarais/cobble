-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local fslib = ...

local function create_tempdir(path)
    local attrs = {
        path = path,
        close = function ()
            fslib.rmdir(path, { recursive = true, ignore_not_found = true })
        end
    }

    local metatable = {
        __close = attrs.close,
        __index = attrs
    }

    return setmetatable({}, metatable)
end

local module_prototype = {
    mkdir = function(...) return fslib.mkdir(...) end,
    rmdir = function(...) return fslib.rmdir(...) end,
    copy = function(...) return fslib.copy(...) end,
    remove = function(...) return fslib.remove(...) end,
    rename = function(oldname, newname) return require("os").rename(oldname, newname) end,
    tempdir = function(...) return create_tempdir(fslib.temp_dir(...)) end
}

local module_metatable = {
    __index = module_prototype
}

return setmetatable({}, module_metatable)
