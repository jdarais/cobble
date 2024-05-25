local on_exit_metatable = {
    __close = function(to_be_closed, err)
        to_be_closed.close_fn(err)
    end
}

local function on_exit (close_fn)
    return setmetatable({
        close_fn = close_fn
    }, on_exit_metatable)
end

local scope_module_prototype = {
    on_exit = on_exit
}

local scope_module_metatable = {
    __index = scope_module_prototype
}

return setmetatable({}, scope_module_metatable)
