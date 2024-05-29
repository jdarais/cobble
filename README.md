# Cobble
Cobble is a multi-project, multi-environment build automation tool

Cobble allows you to define projects and tasks in your repository, which are contained in a single Cobble "workspace".  Tasks can depend on assets and tasks from other projects in the workspace, allowing for the creation of a full-workspace task dependency graph, thus always ensuring that a task's dependencies are up-to-date before running.

Cobble is:

- __Fast__: Cobble is built with technologies that are well-suited for writing fast native applications, including Rust and Lua, and can run tasks in parallel for fast builds.
- __Multi-platform__: Cobble runs on Linux, Mac, and Windows
- __Small__: Download size is <5MB
- __Easy to Install__: Download is a single executable binary
- __Easy to Use__: Tasks are defined in Lua using a simple interface

Note that Cobble is not a build, environment isolation, or package managemetn tool itself, nor does it prescribe any of these.  The examples directory provides some examples of what a workspace could look like using some popular tool configurations.

## Why Cobble?

The world of software project and repository management seems to be stratified between two extremes.  On one extreme, you may find yourself with a sea of small, single-project repositories based on the favored package management and build stack for that language, such as cargo, npm, yarn, poetry, go, etc.  On the other extreme, you may have a monorepo, with all projects combined into one repository, managed by a complex and restrictive monorepo tool such as bazel, pants, or nx.  Often, the ideal lies somewhere in the middle: clusters of projects grouped into repositories based on which projects make sense to "release" together.  The problem, however, is that this paradigm is not well supported by either the single-repository-focused tools or the heavyweight monorepo tools; or if it is supported, it is only for a narrow set of language platforms.  What I've found to be lacking in our current software development ecosystem is a simple, lightweight, general-purpose build automation tool that has a low barrier to entry and can orchestrate tasks across projects within a repository.  Cobble aims to be a tool that fills that gap.

## Cobble Concepts

### Workspace

A workspace is defined by the presence of a `cobble.toml` file at the root of the workspace.  The `cobble.toml` file can contain configuration for the workspace, or it can be empty, simply acting as a marker file to point to the root of the workspace.

### Project

A project is defined in Lua in a `project.lua` file at the root of the project.  There can be many projects in a single workspace.  A project can define tasks, build environments, external tools, or additional sub-projects.

### Task

A task defines a unit of work to be done.  It defines file artifacts and tasks that it depends on, as well as the file artifacts, (if any,) that it produces.  It also defines a list of "Actions" that should be invoked when the task is executed

In the project file, defining a task looks like this:

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

### Action

An action can be defined either as a list of command parameters to be sent to the command line, (or a user-defined "tool"-- more on that later--) or a Lua function.  Actions defined as lists provide a nice shorthand for simple actions, while actions defined as functions grant a lot of power and flexibility in what the action can do.  The action defined in the task above as a list, could also be defined as a function:

```lua
    actions = {
        function (c) c.tool.cmd { "cp", "input.txt", "output.txt" } end
    }
```

### Build Environment

A build environment combines a task, (which sets up the environment,) with an action, (which executes a command within the environment).  Tasks can depend on a build environment, which makes that environment's action available to invoke within the task's actions.  This is helpful when working with build environments that support running commands in an isolated environment, such as npm running commands from a node_modules directory, or python running commands from a virtual environment.  A build environment definition for running commands in a python virtual environment managed by poetry may look something like this:

```lua
env {
    name = "poetry_env",
    install = {
        { tool = "poetry", "install" }
    },
    deps = { files = { "poetry.lock" } },
    action = { tool = "poetry", "run" }
}
```

A task can invoke a build environment's action by specifying the "env" property:

```lua
    actions = {
        { env = "poetry_env", "python", "-m", "pylint", "mypackage/" }
    }
```


### External Tool

A software project's tooling should be as self-contained as possible, (e.g. by leveraging isolated environments to install and run tools,) but at some point, a project has to rely on some tools being externally available to run build and analysis tasks.  Cobble provides a way to define what those external tools are, and optionally define actions that can either check that the tool is installed, or install it if it is not found.  The only required field, though, is an action, which is made available to task and build environment actions that reference the tool.  An external tool definition for poetry might look like this:

```lua
-- Use some preloaded modules provided by Cobble
local cmd = require("cmd")
local version = require("version")

tool {
    name = "poetry",
    check = function (c)
        local res = cmd { "poetry", "--version" }
        assert(res.status == 0, "poetry not found")

        local poetry_version = res.stdout:match("Poetry %(version (%S+)%)")
        assert(version(poetry_version) >= "1.8.0", "Poetry >= 1.8.0 required. Found ".. poetry_version)
    end,
    action = { tool = "cmd", "poetry" }
}
```

### Action Context

Function actions are passed an "Action Context" object, which provides the action with:

- Functions for invoking tools and build environments referenced by the action or the task the action belongs to
- The directory of the project from which the action was invoked
- Information about the dependencies of the task that defined the action
- Functions for sending console output to the buffered output manager
- Args passed to the action (usually from another action)
- The action object itself
