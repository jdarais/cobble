-- Cobble Build Automation
-- Copyright (C) 2024 Jeremiah Darais
--
-- This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

local function ensure_maybe (val)
    if getmetatable(val) == maybe_metatable then
        return val
    else
        return maybe(val)
    end
end

local function maybe_binop (lhs, rhs, op_func)
    return ensure_maybe(lhs):and_then(function (lhs_val)
        return ensure_maybe(rhs):and_then(function (rhs_val)
            return op_func(lhs_val, rhs_val)
        end).value
    end)
end

local maybe;
local maybe_prototype = {
    and_then = function(self, func)
        if self.value == nil then
            return self
        else
            return maybe(func(self.value))
        end
    end,
    or_else = function(self, func)
        if self.value == nil then
            return maybe(func())
        else
            return self
        end
    end
}

local maybe_metatable = {}

function maybe_metatable.__add (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l + r end)
end

function maybe_metatable.__sub (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l - r end)
end

function maybe_metatable.__mul (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l * r end)
end

function maybe_metatable.__div (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l / r end)
end

function maybe_metatable.__mod (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l % r end)
end

function maybe_metatable.__pow (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l ^ r end)
end

function maybe_metatable.__unm (val)
    return val:and_then(function (v) return -v end)
end

function maybe_metatable.__idiv (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l // r end)
end

function maybe_metatable.__band (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l & r end)
end

function maybe_metatable.__bor (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l | r end)
end

function maybe_metatable.__bxor (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l ~ r end)
end

function maybe_metatable.__bnot (val)
    return val:and_then(function (v) return ~v end)
end

function maybe_metatable.__shl (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l << r end)
end

function maybe_metatable.__shr (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l >> r end)
end

function maybe_metatable.__concat (lhs, rhs)
    return maybe_binop(lhs, rhs, function (l, r) return l .. r end)
end

function maybe_metatable.__len (val)
    return val:and_then(function (v) return #v end)
end

function maybe_metatable.__eq (lhs, rhs)
    return ensure_maybe(lhs)
        :and_then(function (lhs_val)
            return ensure_maybe(rhs)
                :and_then(function (rhs_val) return lhs_val == rhs_val end)
                :or_else(function () return false end)
                .value
        end)
        :or_else(function ()
            return ensure_maybe(rhs)
                :and_then(function (_) return false end)
                :or_else(function () return true end)
                .value
        end)
        .value
end

function maybe_metatable.__lt (lhs, rhs)
    return ensure_maybe(lhs)
        :and_then(function (lhs_val)
            return ensure_maybe(rhs)
                :and_then(function (rhs_val) return lhs_val < rhs_val end)
                :or_else(function () return false end)
                .value
        end)
        :or_else(function ()
            return ensure_maybe(rhs)
                :and_then(function (_) return true end)
                :or_else(function () return false end)
                .value
        end)
        .value
end

function maybe_metatable.__le (lhs, rhs)
    return ensure_maybe(lhs)
        :and_then(function (lhs_val)
            return ensure_maybe(rhs)
                :and_then(function (rhs_val) return lhs_val <= rhs_val end)
                :or_else(function () return false end)
                .value
        end)
        :or_else(function ()
            return ensure_maybe(rhs)
                :and_then(function (_) return true end)
                :or_else(function () return true end)
                .value
        end)
        .value
end

function maybe_metatable.__index (maybe_tbl, key)
    -- If we get a request for the "value" key here, then the value is nil
    if key == "value" then
        return nil
    end

    return maybe_prototype[key] or maybe_tbl:and_then(function (tbl) return tbl[key] end)
end

-- note: __newindex is not supported

function maybe_metatable.__call (maybe_func, ...)
    local args = table.pack(...)
    return maybe_func:and_then(function (func) return func(table.unpack(args)) end)
end

maybe = function(value)
    return setmetatable({
        value = value
    }, maybe_metatable)
end

return maybe
