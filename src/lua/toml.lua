local toml_lib = ...

toml = {
    loads = function (...) return toml_lib.loads(...) end,
    read = function (...) return toml_lib.read(...) end
}
