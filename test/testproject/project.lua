

external_tool {
    name = "poetry",
    install = function ()
        -- TODO: Implement this
    end,
    check = function ()
        -- TODO: Implement this
    end,
    action = {
        tool = "bla",
        function(cxt)
            return cxt.cmd { "poetry", table.unpack(cxt.args) }
        end
    }
}

project_dir("subproject")
