

external_tool {
    name = "poetry",
    install = function ()
        -- TODO: Implement this
    end,
    check = function ()
        -- TODO: Implement this
    end,
    action = {
        exec = function(self, env, args)
            return env:exec { "poetry", table.unpack(args) }
        end
    }
}
