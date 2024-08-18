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

#### Task Dependencies and Artifacts

Task dependencies are declared using the `deps` property.  A task can depend on files, other tasks, or variables.  Additionally, a task can declare any files that it generates as artifacts.  When a task is selected to be run, if none of the dependencies have changed since the last time a task was run, and the artifact files' content hashes match what was output by the last run, the task will be considered "up to date".  If a task selected to run is found to be up to date, it will simply be skipped, and the last output of the task will be used.

#### Task Outputs

A task also produces an "output", which is the value returned by the last action in the task's actions list. The outputs of a task can be used for various purposes, including dynamically calculated dependencies, or as input to other tasks.

### Action

An action can be defined either as a list of command parameters to be sent to the command line, (or a user-defined "tool": more on that later,) or a Lua function.  Actions defined as lists provide a nice shorthand for simple actions, while actions defined as functions grant a lot of power and flexibility in what the action can do.  The action defined in the task above as a list could also be defined as a function:

```lua
    actions = {
        function (c) c.tool.cmd { "cp", "input.txt", "output.txt" } end
    }
```

### Action Context

Function actions are passed an "Action Context" object, which provides the action with declared tools and environments, as well as other information related to dependencies and the project context.

#### Action Context Properties

- __tool__: A table mapping tool names to tool invocation functions. Contains an entry for each tool available to the action
- __env__: A table mapping build environment names to invocation functions. Contains an entry for each build environment available to the action
- __action__: A copy of the action definition that defines the current action
- __files__: A table containing file dependencies of the task that the action belongs to.  Table keys are file paths as they are declared in the task.  Each value is a table containing a `path` and `hash` property, which provides both the path of the file relative to the workspace root and the fingerprint hash of the file contents.
- __tasks__: A table containing information about task dependencies of the task that the action belongs to.  Table keys are task dependency names as they were declared in the action's task.  Each value is the output returned by the last action in the task dependency.
- __vars__: A table containing information about variable dependencies of the task that the action belongs to.  Table keys are variable names as they were declared in the action's task.   Each value is the value of that variable.
- __project__: A table containing information about the project in which the action's task was defined.  The table currently has a single property, `dir`, which provide's the project's directory relative to the workspace root.
- __args__: A table containing arguments passed to the action.  If the action is part of a tool or build environment being invoked, `args` contains the arguments passed to the tool or build environment.  If the action belongs to a task, `args` contains the return value of the previous action executed in the same task, if there is one.
- __out__: A function to send text to stdout.  (Note that this function is preferred over the Lua `print` function, since it manages buffering and ensuring that a task's output gets printed out together in the console, instead of being interleaved with the output of other tasks being run in parallel.)
- __err__: A function to send text to stderr.  (Note that this function is preferred over the Lua `print` function for the same reasons as with the `out` function.)

### Build Environment

A build environment combines an install task, which sets up the environment, with an action, which can be used to invoke commands in that environment.  If an action references a build environment, the build environment's install task is automatically added as a dependency to the task that the action belongs to.  This is helpful when working with build environments that support running commands in an isolated environment, such as npm or python virtual environments.  A build environment definition for running commands in a python virtual environment managed by poetry may look something like this:

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

A task can invoke a build environment's action by specifying the "env" property:

```lua
task {
    name = "lint",
    actions = {
        { env = "poetry_env", "python", "-m", "pylint", "mypackage/" }
    }
}
```


### External Tool

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

### Variables

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

With this variable and build environment setup, you can simply change the python version number in the workspace cobble.toml and run whatever task you like.  If a task depends on the "poetry_env" environment, Cobble will ensure that the setup task is re-run, setting up the virtual environment with the correct python version, before running the task.

### Project and Task Names

Project names reflect the directory structure in which they exist.  For example, if a project exists in a directory `projects/project_a/` within a workspace, then the project name will be `/projects/project_a`.  A `project.lua` file can declare additional projects either by calling `project_dir` with a subdirectory, or by calling `project` with a project name and additional task, environment, or tool definitions.  When delcaring a project with the `project` function, the project name is the name provided in the `project` call, appended to the name of the project in which the subproject is declared.

Task names function similarly.  A task's full name is the name provided in the task declaration, appended to the name of the project in which the task is declared.  For example, if the project `/projects/project_a` declares a task named `task_b`, the full name of the task is `/projects/project_a/task_b`.

### Task and File References

In a Cobble workspace, tasks in any project can reference tasks or files anywhere in the workspace, including in other projects.  All task and file references found in a task declaration are interpreted as being relative to the project in which the task was declared.  Task references can include `..` path elements that traverse one level up in the task hierarchy.  Task references starting with `/` are interpreted as absolute task name references.

File paths can also be provided as an absolute path.  To specify an absolute file path, it can be helpful to use the `WORKSPACE.dir` global variable that is provided by Cobble, which provides the absolute path to the workspace root directory.

#### Current Working Directory and File References

Note that while task and file references in a task declaration are interpreted as being relative to the project directory, the CWD used when running any task is the workspace root.  Since tasks run in parallel threads within the same process, Cobble is not able to set a CWD for each task.  This means that within a function-based action implementation, any functions that directly interact with files will interpret relative file paths as being relative to the workspace root, not the project.  You can use the action context variable `c.project.dir` to get the project directory relative to the workspace root.

Paths passed as arguments to environments or tools are generally still interpreted as being relative to the project directory.  Bulit-in tools like the `cmd` tool follow this behavior, and user-defined environments and tools should follow this pattern as well. 

### Modules

Cobble sets the workspace root as the module path root for including modules.  This means that if you have a lua module that exists at `cobble_modules/python.lua`, you can include that in any lua script with `require("cobble_modules.python")`.  Leveraging modules is a great way to share logic for defining different types of projects.

Note that using native modules may work, but is not officially supported.  Cobble is designed to make workspaces as portable and self-contained as possible, and use of native lua modules runs counter to this philosophy.

#### Cobble modules

Cobble provides additional modules on top of the Lua standard library to provide extra functionality.  For details on the modules that Cobble provides, see the API reference.

## API Reference

### Globals

The following global variables are available in any lua environment:

- __WORKSPACE__: a table containing information about the workspace:
  - __dir__: absolute path to the workspace directory
- __PLATFORM__: a table containing information about the platform
  - __arch__: the platform architecture, as detected by Rust in [std::env::consts::ARCH](https://doc.rust-lang.org/std/env/consts/index.html)
  - __os_family__: the platform OS family, as detected by Rust in [std::env::consts::FAMILY](https://doc.rust-lang.org/std/env/consts/constant.FAMILY.html)
  - __os__: the platform OS, as detected by Rust in [std::env::consts::OS](https://doc.rust-lang.org/std/env/consts/constant.OS.html)

### Cobble Modules

#### Module `cmd`

The `cmd` module is a function that invokes a command, providing some additional functionality over Lua's `os.execute` function, as well as integration with Cobble actions.

- `cmd(args)`
  - Arguments:
    - `args`: table
      - `cwd`: string | nil - Current working directory to run the command with
      - `out`: string | nil - Callback to be called with any stdout output
      - `err`: string | nil - Callback to be called with any stderr output
      - `...` (numeric keys): string - Any positional (numeric index) table elements are interpreted as the command and command args to execute
  - Return value: table
    - `status`: int - The return status of the launched process
    - `stdout`: string - The stdout output of the process
    - `stderr`: string - The stderr output of the process

#### Module `path`

The `path` module contains basic path manipulation functionality.

- `path.SEP`: string - The path separator character for the current OS
- `path.glob([base], pattern)`: function - Get files matching a pattern in a directory tree
  - Arguments:
    - `base`: (optional) string - Base path to search from.  Returned file paths are relative to the base path.  (Default = CWD)
    - `pattern`: string - Pattern to match files with.  Can include `*` or `**` wildcards.
  - Return Value: table - A list of paths found that match the given pattern
    - `...` (nuumeric keys): string
- `path.join(...)`: function - Join path segments using the OS-specific path separator
  - Arguments:
    - `...` (positional args): string - path segments to join
  - Return Value: string - the joined path
- `path.is_dir(path)`: function - Determine whether the path exists and is a directory
  - Arguments:
    - `path`: string - The path to test
  - Return Value: boolean
- `path.is_file(path)`: function - Determine whether the path exists and is a file
  - Arguments:
    - `path`: string - The path to test
  - Return Value: boolean

#### Module `iter`

The `iter` module provides a convenient, functional interface for manipulating lists lazily and efficiently

- `iter(iter_fn, state, init_ctrl_var, close)`: function - wrap a set of iterator functions in an `iter` object.
  - Arguments: This constructor function is intended to be used with Lua's `ipairs` or `pairs` functions, or any source of values intended to used with Lua's [generic for](https://www.lua.org/manual/5.4/manual.html#3.3.5) loop.  (Example: `local it = iter(ipairs(some_list)))`)
  - Return Value: iter object





