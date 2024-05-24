function extend(target, source, start_index)
    local start_offset = (start_index or (#target+1)) - 1

    for k, v in pairs(source) do
        if type(k) == "number" then
            target[k+start_offset] = v
        else
            target[k] = v
        end
    end

    return target
end
