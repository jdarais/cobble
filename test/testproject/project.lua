

external_tool {
    name = "poetry",
    install = function ()
        -- TODO: Implement this
    end,
    check = function ()
        -- TODO: Implement this
    end,
    action = {
        tool = "cmd",
        function(c)
            return c.tool.cmd { "poetry", table.unpack(c.args) }
        end
    }
}

project_dir("subproject")
