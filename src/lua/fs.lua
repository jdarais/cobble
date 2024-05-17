local fslib = ...

fs = {
    glob = function(...) return fslib.glob(...) end,
    is_dir = function(...) return fslib.is_dir(...) end,
    is_file = function(...) return fslib.is_file(...) end
}
