
function iter(it_func, state, init_ctrl_var, close)
    return {
        it_func = it_func,
        state = state,
        init_ctrl_var = init_ctrl_var,
        close = close,
        map = function(self, map_func)
            return iter(map(map_func, self.it_func, self.state, self.init_ctrl_var, self.close))
        end,
        enumerate = function(self)
            return iter(enumerate(self.it_func, self.state, self.init_ctrl_var, self.close))
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
        for_each = function(self, func)
            for v1, v2, v3, v4, v5, v6, v7, v8, v9 in self:iterate() do
                func(v1, v2, v3, v4, v5, v6, v7, v8, v9)
            end
        end,
        to_table = function(self)
            return self:reduce({}, function(accum, k, v) accum[k] = v ; return accum end)
        end
    }
end

function map (map_func, it_func, state, init_ctrl_var, close)
    local inner_ctrl_var = init_ctrl_var  
    local map_it_func = function (st, _v)
        local v1, v2, v3, v4, v5, v6, v7, v8, v9 = it_func(st, inner_ctrl_var)
        if v1 == nil then
            return nil
        end

        inner_ctrl_var = v1

        return map_func(v1, v2, v3, v4, v5, v6, v7, v8, v9)
    end

    return map_it_func, state, init_ctrl_var, close
end

function enumerate (it_func, inner_state, init_ctrl_var, close)
    local inner_ctrl_var = init_ctrl_var

    local enumerate_it_func = function (_st, i)
        local v1, v2, v3, v4, v5, v6, v7, v8, v9 = it_func(inner_state, inner_ctrl_var)
        if v1 == nil then
            return nil
        end

        inner_ctrl_var = v1
        return i + 1, v1, v2, v3, v4, v5, v6, v7, v8, v9
    end

    return enumerate_it_func, state, 0, close
end

function filter (filter_func, it_func, state, init_ctrl_var, close)
    local filter_it_func = function (st, ctrl_var)
        local v1, v2, v3, v4, v5, v6, v7, v8, v9 = (ctrl_var)
        while true do
            v1, v2, v3, v4, v5, v6, v7, v8, v9 = it_func(st, v1)
            if v1 == nil then
                return nil
            end
            if filter_func(v1, v2, v3, v4, v5, v6, v7, v8, v9) then
                return v1, v2, v3, v4, v5, v6, v7, v8, v9
            end
        end
    end

    return filter_it_func, state, init_ctrl_var, close
end

function reduce (init_accum, reduce_func, it_func, state, init_ctrl_var, close)
    local accum = init_accum
    for v1, v2, v3, v4, v5, v6, v7, v8, v9 in it_func, state, init_ctrl_var, close do
        accum = reduce_func(accum, v1, v2, v3, v4, v5, v6, v7, v8, v9)
    end

    return accum
end
