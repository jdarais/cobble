task {
    name = "task1",
    actions = {
        { "sleep", "1" },
        { "echo", "Task 1!!" },
        { "false" },
    }
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
    actions = {
        { "echo", "Task 3!!" },
    }
}
