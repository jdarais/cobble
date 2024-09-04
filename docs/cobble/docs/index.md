# About Cobble

## What is Cobble?

Cobble is a multi-project, multi-environment build automation tool.

Cobble allows you to define tasks across a collection of projects, which are contained in a single Cobble "workspace".  Tasks can depend on assets and tasks from other projects in the workspace, allowing for the creation of a full-workspace task dependency graph.  When executing any task, Cobble ensures that all of the task's dependencies have been executed first.

Cobble is:

- __Fast__: Cobble is built with technologies that are well-suited for writing fast native applications, including Rust and Lua, and can run tasks in parallel for fast builds.
- __Flexible__: Cobble is easy to customize to work with existing project structures and build environments.  Build a monorepo without sacrificing IDE integration.
- __Cross-platform__: Cobble runs on Linux, Mac, and Windows
- __Small__: Download size is <5MB
- __Easy to Install__: Download is a single executable binary, with no library or script environment dependencies
- __Easy to Use__: Tasks are defined in Lua using a simple interface

## Cobble Tasks

Tasks are written in Lua, and can be used to automate just about any type of build step:

```lua
task {
    name = "copy_file",
    deps = { files = { "input.txt" } },
    artifacts = { files = { "output.txt" } },
    actions = {
        { "cp", "input.txt", "output.txt" }
    }
}
```

## Why Cobble?

Software project tooling often centeres around a core tool, (such as npm, cargo, go,) that provides features such as package management, environment isolation, and task execution.  For many simple projects, the task execution capabilities of the core tool, if they exist, are sufficient.  For projects that require multiple build steps, that combine multiple language platforms, or for which the core tool doesn't provide any task execution features, a separate build automation tool is useful.  Existing build automation tools range from simple tools like make or doit, to more complex monorepo tools, like bazel or pants.  Cobble aims to fill a gap in the middle: it is a general purpose build automation tool that has a simple language and a low barrier to entry, but is also monorepo-aware, making it easy to define tasks and dependencies across multiple projects.
