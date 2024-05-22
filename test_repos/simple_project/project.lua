task {
    name = "task1",
    actions = {
        function (c) c.tool.cmd { "echo", "hi" } end,
        function (c)
            local result = c.tool.cmd { "echo", "there" }
            io.open(c.project.dir .. "/output.txt", "w")
                :write(result.stdout)
                :close()
        end
    },
    deps = {
        vars = { "this.var" }
    },
    artifacts = {
        "output.txt" 
    }
}

project {
    name = "time",
    def = function ()
        task {
            name = "take_time",
            actions = {
                function (c) c.tool.cmd { "bash", c.files["script"].path } end
            },
            deps = {
                files = { script = "take_time.sh" }
            }
        }
    end
}

task {
    name = "task2",
    deps = {
        tasks = { "task1", "task3" }
    },
    actions = {
        { "echo", "Task 2!!" }
    }
}

task {
    name = "task3",
    default = true,
    actions = {
        { "echo", "Task 3!!" },
    }
}
