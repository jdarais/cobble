local on_scope_exit_metatable = {
    __close = function(to_be_closed, err)
        to_be_closed.close_fn(err)
    end
}

function on_scope_exit (close_fn)
    local to_be_closed = {
        close_fn = close_fn
    }
    setmetatable(to_be_closed, to_be_closed_metatable)
    return to_be_closed
end
