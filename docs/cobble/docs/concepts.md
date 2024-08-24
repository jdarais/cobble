# Cobble Concepts

## Workspace

A workspace is defined by the presence of a `cobble.toml` file at the root of the workspace.  The `cobble.toml` file can contain configuration for the workspace, or it can be empty, simply acting as a marker file to point to the root of the workspace.

```
An example, minimal Cobble workspace directory structure:
workspace_dir/
  |- proj_a/
  |    `- project.lua
  |- proj_b/
  |    `- project.lua
  |- cobble.toml
  `- project.lua
```

## Projects

A project is defined in Lua in a `project.lua` file at the root of the project.  There can be many projects in a single workspace.  A project can define tasks, environments, external tools, or additional sub-projects.

## Tasks

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

### Task Dependencies and Artifacts

Task dependencies are declared using the `deps` property.  A task can depend on files, other tasks, or variables.  Additionally, a task can declare any files that it generates as artifacts.  When a task invoked with `cobl run`, its full dependency tree is scanned.  Any tasks that are directly depended on, or that declare files that are depended on as artifacts, will also be selected to run.  All task ependencies are executed first before executing the task that depends on them.

When a task is selected to be run, if none of the dependencies have changed since the last time a task was run, and the artifact files' content hashes match what was output by the last run, the task will be considered "up to date".  If a task selected to run is found to be up to date, it will simply be skipped, and the last output of the task will be used.

### Task Outputs

A task also produces an "output", which is the value returned by the last action in the task's actions list. The outputs of a task can be used for various purposes, including dynamically calculated dependencies, or as input to other tasks.

### Calculated Dependencies and Artifacts

Dependencies for a task can be calculated dynamically.  This is useful, for example, if the dependency information for a task is contained in a project file or requires scanning a directory for files.

## Actions

An action can be defined either as a list of command parameters to be sent to the command line, (or a user-defined "tool": more on that later,) or a Lua function.  Actions defined as lists provide a nice shorthand for simple actions, while actions defined as functions grant a lot of power and flexibility in what the action can do.  The action defined in the task above as a list could also be defined as a function:

```lua
actions = {
    function (c) c.tool.cmd { "cp", "input.txt", "output.txt" } end
}
```

### Action Context

Function actions are passed an "Action Context" object, which provides the action with declared tools and environments, as well as other information related to dependencies and the project context.  For example, all of the tools available to the action can be accessed in the context's `tool` property, as demonstrated in the action above.

## Environments

An environment combines a setup task, which sets up the environment, with an action, which can be used to invoke commands in that environment.  If an action references an environment, the environment's setup task is automatically added as a dependency to the task that the action belongs to.  This is helpful when working with environments that support running commands in an isolated environment, such as npm or python virtual environments.  An environment definition for running commands in a python virtual environment managed by poetry may look something like this:

```lua
env {
    name = "poetry_env",
    setup_task = {
        actions = {
            { tool = "poetry", "install" }
        }
    },
    deps = { files = { "poetry.lock" } },
    action = { tool = "poetry", "run" }
}
```

A task can invoke an environment's action by specifying the "env" property:

```lua
task {
    name = "lint",
    actions = {
        { env = "poetry_env", "python", "-m", "pylint", "mypackage/" }
    }
}
```


## External Tools

A software project's tooling should be as self-contained as possible, (e.g. by leveraging isolated environments to install and run tools,) but at some point, a project has to rely on some tools being externally available to run build and analysis tasks.  Cobble provides a way to define what those external tools are, and optionally define an action to check that the tool is correctly installed.  External tool definitions also provide a convenient abstraction layer, where you can include platform-specific tool invocation logic so that the tasks throughout your project don't have to.  The action property on a tool defines what the tool does when invoked.  An external tool definition for poetry might look like this:

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
    action = { "poetry" }
}
```

## Variables

Variables can be defined in the top-level cobble.toml file, and tasks can declare a dependency on specific variables.  When a variable that a task depends on changes, that task will be re-run.

Here's an example of what declaring a variable might look like to specify a python version to use for the workspace:

```toml
# cobble.toml
[vars]
python.version = "3.10"
```

```lua
-- project.lua
env {
    name = "poetry_env",
    setup_task = {
        actions = {
            { tool = "poetry", function (c) c.tool.poetry { "env", "use", c.vars["python.version"] } end },
            { tool = "poetry", "install" }
        },
        deps = {
            vars = { "python.version" }
        }
    },
    action = { tool = "poetry", "run" }
}
```

With this variable and environment setup, you can simply change the python version number in the workspace cobble.toml and run whatever task you like.  If a task depends on the "poetry_env" environment, Cobble will ensure that the setup task is re-run, setting up the virtual environment with the correct python version, before running the task.

## Project and Task Names

Project names reflect the directory structure in which they exist.  For example, if a project exists in a directory `projects/project_a/` within a workspace, then the project name will be `/projects/project_a`.  A `project.lua` file can declare additional subprojects, who's names are prefixed with the name of the project in which they are declared.  For example, if the above `projects/project_a/project.lua` file calls `project("subproject_b", project_def_fn)`, the resulting subproject is named `/projects/project_a/subproject_b`.

Task names function similarly.  A task's full name is the name provided in the task declaration, appended to the name of the project in which the task is declared.  For example, if the project `/projects/project_a` declares a task named `task_b`, the full name of the task is `/projects/project_a/task_b`.

Full names of tasks and projects must be unique across all tasks and projects in the workspace.

## Task and File References

In a Cobble workspace, tasks in any project can reference tasks or files anywhere in the workspace, including in other projects.  All task and file references found in a task declaration are interpreted as being relative to the project in which the task was declared.  Task references can include `..` path elements that traverse one level up in the task hierarchy.  Task references starting with `/` are interpreted as absolute task name references.

File paths can also be provided as an absolute path.  To specify an absolute file path, it can be helpful to use the `WORKSPACE.dir` global variable that is provided by Cobble, which provides the absolute path to the workspace root directory.

### Current Working Directory and File References

Note that while task and file references in a task declaration are interpreted as being relative to the project directory, the CWD used when running any task is the workspace root.  Since tasks run in parallel threads within the same process, Cobble is not able to set a CWD for each task.  This means that within a function-based action implementation, any functions that directly interact with files will interpret relative file paths as being relative to the workspace root, not the project.  You can use the action context variable `c.project.dir` to get the project directory relative to the workspace root.

Note that the built-in `cmd` tool interprets file paths as relative to the project directory.  It is recommended for user-defined tools and environments to do so as well. 

## Modules

Cobble sets the workspace root as the module path root for including modules.  This means that if you have a lua module that exists at `cobble_modules/python.lua`, you can include that in any lua script with `require("cobble_modules.python")`.  Leveraging modules is a great way to share logic for defining different types of projects.

Note that using native modules may work, but is not officially supported.  Cobble is designed to make workspaces as portable and self-contained as possible, and use of native lua modules runs counter to this philosophy.

### Cobble modules

Cobble provides additional modules on top of the Lua standard library to provide extra functionality.  For details on the modules that Cobble provides, see the [Built-in Modules](modules.md) reference.
