task {
    name = "task1",
    actions = {
        { "echo", "Task 1!!" }
    }
}

task {
    name = "task2",
    deps = {
        tasks = { "task1" }
    },
    actions = {
        { "echo", "Task 2!!" }
    }
}
