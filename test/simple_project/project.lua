task {
    name = "task1",
    actions = {
        { "echo", "Task 1!!" },
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
