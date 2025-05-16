task {
    name = "setup",
    always_run = true,
    actions = {
        function (c) c.println("INITIALIZATION!!!") end
    }
}
