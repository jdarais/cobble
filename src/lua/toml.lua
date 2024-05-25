local toml_lib = ...

toml = {
    loads = function (...) return toml_lib.loads(...) end,
    load = function (...) return toml_lib.load(...) end,
    dumps = function (...) return toml_lib.dumps(...) end,
    dump = function (...) return toml_lib.dump(...) end,
}
