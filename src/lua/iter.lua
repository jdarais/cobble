
function iter(it_func, state, init_ctrl_var, close)
    return {
        it_func = it_func,
        state = state,
        init_ctrl_var = init_ctrl_var,
        close = close,
        map = function(self, map_func)
            return iter(map(map_func, self.it_func, self.state, self.init_ctrl_var, self.close))
        end,
        filter = function(self, filter_func)
            return iter(filter(filter_func, self.it_func, self.state, self.init_ctrl_var, self.close))
        end,
        reduce = function(self, init_accum, reduce_func)
            return reduce(init_accum, reduce_func, self.it_func, self.state, self.init_ctrl_var, self.close)
        end,
        iterate = function(self)
            return self.it_func, self.state, self.init_ctrl_var, self.close
        end,
        to_list = function(self)
            return self:reduce({}, function(accum, k, v) table.insert(accum, k, v) return accum end)
        end
    }
end

function map (map_func, ...)
    local it_func, state, init_ctrl_var, close = ...
    
    local map_it_func = function (st, ctrl_var)
        local inner_next = {it_func(st, ctrl_var)}
        if inner_next[1] == nil then
            return nil
        end
        local mapped_next = {map_func(table.unpack(inner_next))}
        return inner_next[1], table.unpack(mapped_next)
    end

    return map_it_func, state, init_ctrl_var, close
end

function filter (filter_func, ...)
    local it_func, state, init_ctrl_var, close = ...
    
    local filter_it_func = function (st, ctrl_var)
        local inner_next = { ctrl_var }
        repeat
            inner_next = {it_func(st, inner_next[1])}
            if filter_func(table.unpack(inner_next)) then
                return table.unpack(inner_next)
            end
        until inner_next[1] == nil
        -- We reached the end of the iterator
        return table.unpack(inner_next)
    end

    return filter_it_func, state, init_ctrl_var, close
end

function reduce (init_accum, reduce_func, ...)
    local it_func, state, init_ctrl_var, close = ...

    local accum = init_accum
    local next_val = {it_func(state, init_ctrl_var)}
    while next_val[1] ~= nil do
        accum = reduce_func(accum, table.unpack(next_val))
        next_val = {it_func(state, next_val[1])}
    end

    return accum
end
