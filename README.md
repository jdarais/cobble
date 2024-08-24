# Cobble
Cobble is a multi-project, multi-environment build automation tool

Cobble allows you to define projects and tasks in your repository, which are contained in a single Cobble "workspace".  Tasks can depend on assets and tasks from other projects in the workspace, allowing for the creation of a full-workspace task dependency graph, thus always ensuring that a task's dependencies are up-to-date before it runs.

Cobble is:

- __Fast__: Cobble is built with technologies that are well-suited for writing fast native applications, including Rust and Lua, and can run tasks in parallel for fast builds.
- __Multi-platform__: Cobble runs on Linux, Mac, and Windows
- __Small__: Download size is <5MB
- __Easy to Install__: Download is a single executable binary
- __Easy to Use__: Tasks are defined in Lua using a simple interface

Note that Cobble is not a build, environment isolation, or package management tool itself, nor does it prescribe any of these.  The examples directory provides some examples of what a workspace could look like using some popular tool configurations.

## Why Cobble?

The world of software project and repository management seems to be stratified between two extremes.  On one extreme, you may find yourself with a sea of small, single-project repositories based on the favored package management and build stack for that language, such as cargo, npm, yarn, poetry, go, etc.  On the other extreme, you may have a monorepo, with all projects combined into one repository, managed by a complex and restrictive monorepo tool such as bazel, pants, or nx.  Often, the ideal lies somewhere in the middle: clusters of projects grouped into repositories based on which projects make sense to "release" together.  The problem, however, is that this paradigm is not well supported by either the single-repository-focused tools or the heavyweight monorepo tools; or if it is supported, it is only for a narrow set of language platforms.  What I've found to be lacking in our current software development ecosystem is a simple, lightweight, general-purpose build automation tool that has a low barrier to entry and can orchestrate tasks across projects within a repository.  Cobble aims to be a tool that fills that gap.

## Cobble Concepts

### Workspace

A workspace is defined by the presence of a `cobble.toml` file at the root of the workspace.  The `cobble.toml` file can contain configuration for the workspace, or it can be empty, simply acting as a marker file to point to the root of the workspace.

### Projects

A project is defined in Lua in a `project.lua` file at the root of the project.  There can be many projects in a single workspace.  A project can define tasks, build environments, external tools, or additional sub-projects.

### Tasks

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

Task dependencies are declared using the `deps` property.  A task can depend on files, other tasks, or variables.  Additionally, a task can declare any files that it generates as artifacts.  When a task invoked with `cobl run`, its full dependency tree is scanned.  Any tasks that are directly depended on, or that declare files that are depended on as artifacts, will also be selected to run.  All task ependencies are executed first before executing the task that depends on them.

When a task is selected to be run, if none of the dependencies have changed since the last time a task was run, and the artifact files' content hashes match what was output by the last run, the task will be considered "up to date".  If a task selected to run is found to be up to date, it will simply be skipped, and the last output of the task will be used.

#### Task Outputs

A task also produces an "output", which is the value returned by the last action in the task's actions list. The outputs of a task can be used for various purposes, including dynamically calculated dependencies, or as input to other tasks.

#### Calculated Dependencies and Artifacts

Dependencies for a task can be calculated dynamically.  This is useful, for example, if the dependency information for a task is contained in a project file or requires scanning a directory for files.

### Actions

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

### Build Environments

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


### External Tools

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

Note that the built-in `cmd` tool interprets file paths as relative to the project directory.  It is recommended for user-defined tools and build environments to do so as well. 

### Modules

Cobble sets the workspace root as the module path root for including modules.  This means that if you have a lua module that exists at `cobble_modules/python.lua`, you can include that in any lua script with `require("cobble_modules.python")`.  Leveraging modules is a great way to share logic for defining different types of projects.

Note that using native modules may work, but is not officially supported.  Cobble is designed to make workspaces as portable and self-contained as possible, and use of native lua modules runs counter to this philosophy.

#### Cobble modules

Cobble provides additional modules on top of the Lua standard library to provide extra functionality.  For details on the modules that Cobble provides, see the API reference.

## API Reference

### Globals

The following global variables are available in any lua environment:

- `WORKSPACE`: a table containing information about the workspace:
  - `dir`: absolute path to the workspace directory
- `PLATFORM`: a table containing information about the platform
  - `arch`: the platform architecture, as detected by Rust in [std::env::consts::ARCH](https://doc.rust-lang.org/std/env/consts/index.html)
  - `os_family`: the platform OS family, as detected by Rust in [std::env::consts::FAMILY](https://doc.rust-lang.org/std/env/consts/constant.FAMILY.html)
  - `os`: the platform OS, as detected by Rust in [std::env::consts::OS](https://doc.rust-lang.org/std/env/consts/constant.OS.html)

#### Project Definition Globals (Only available during project definition phase)

- `task(task_def)`: function - define a task
  - `task_def`: table - task definition properties
    - `name`: string - the task name
    - `default`: bool | nil - whether the task is a default task for the project.  When `cobl run` is given a project name, the default tasks for that project are run.  If no tasks are defined as default for a project, passing the project name to `cobl run` runs all tasks in the project. (default=false)
    - `always_run`: bool | nil - If true, the task will always be run if selected, regardless of whether its dependencies and artifacts are up-to-date. (default=false)
    - `interactive`: bool | nil - If true, child processes launched by this task can attach to stdin.  Note that interactive tasks cannot run in parallel. (default=false)
    - `stdout`: "always" | "never" | "on_fail" | nil - When to display stdout output from the task (default="on_fail")
    - `stderr`: "always" | "never" | "on_fail" | nil - When to display stderr output from the task (default="on_fail")
    - `output`: "always" | "never" | "on_fail" | nil - Setting this property will set both `stdout` and `stderr` properties.  If either `stdout` or `stderr` properties are present, they will take precedence over the value provided by `output`.
    - `env`: string | table | nil - If provided, the named environment is available to all actions in the task.  A table mapping an environment alias to an environment name is also valid, however only a single environment can be specified at the task level.
    - `actions`: table - A list of actions that contain the execution logic for the task.
    - `clean`: table | nil - A list of actions to run when the task is selected in a `cobl clean` command.
    - `deps`: table | nil - A mapping of dependency type to a list of dependencies
      - `files`: table | nil - A list of file dependency paths
      - `tasks`: table | nil - A list of task dependency names
      - `calc`: table | nil - A list of tasks to execute for calculating dependencies.  The calc task's output, (i.e. the return value of the tasks last action,) should match the same structure as the `deps` property for task definitions, with the exception that calc dependencies producing additional calc dependencies is not supported.  Calculated results will be combined and added to the statically declared dependencies.
    - `artifacts`: table | nil - A mapping of artifact type to a list of artifacts
      - `files`: table | nil - A list of file artifact paths
      - `calc`: table | nil - A list of tasks to execute for calculating artifacts.  The calc task's output should be a list of file paths.
- `env(env_def)`: function - define a build environment
  - `env_def`: table - build environment definition properties
    - `name`: string - the build environment name
    - `setup_task`: `task_def` - the task to execute to set up the build environment, (e.g. "npm install").  All `task_def` properties are supported except for `name`.  The setup task will be given the same name as the build environment.
    - `action`: table | function - an action that will run a command in the build environment, (e.g. "npm exec").  For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.
- `tool(tool_def)`: function - define an external tool
  - `tool_def`: table - tool definition properties
    - `name`: string - the tool name.  Unlike tasks and build environments, tool names are global, and are not combined with a project name to create a full name.
    - `check`: table - an action for checking whether the tool is correctly installed, (correct version, etc.).  If the check fails, the check action should call `error` to raise an error.
    - `action`: table | function - an action that will execute the tool. For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.
- `project_dir(path)`: function - add a project in a subdirectory
  - `path`: string - the path to the project
- `project(name, project_def_cb)`: function - add a named project in the same directory as the current project 
  - `name`: string - the name of the subproject
  - `project_def_cb`: function - a function definition that, when called, creates `task`, `env`, and `tool` definitions for the subproject.

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

The `iter` module provides a convenient, functional interface for manipulating lists lazily and efficiently.

- `iter(iter_fn, state, init_ctrl_var, close)`: function - wrap a set of iterator functions in an `iter` object.
  - Arguments: This constructor function is intended to be used with Lua's `ipairs` or `pairs` functions, or any source of values intended to used with Lua's [generic for](https://www.lua.org/manual/5.4/manual.html#3.3.5) loop.  (Example: `local it = iter(ipairs(some_list)))`)
  - Return Value: iter object
- `iter.map(map_fn, iter_fn, state, init_ctrl_var, close)`: function - apply a map operation to an iterator
  - Arguments:
    - `map_fn`: function - a function that takes in a value produced by the original iterator, and returns a new value
    - `iter_fn, state, init_ctrl_var, close`: iterator vars - variable sequence provided by `ipairs`, `pairs`, or any other source of values intended to be used with Lua's generic for loop.
- `iter.enumerate(iter_fn, state, inti_ctrl_var, close)`: function - add an index at the beginning of the value sequence returned by the iterator.  For example, if the third value the original iterator produces is "a", `enumerate` will produce an iterator that returns 3, "a" as its third value.
  - Arguments:
    - `iter_fn, state, init_ctrl_var, close`: iterator vars - variable sequence provided by `ipairs`, `pairs`, or any other source of values intended to be used with Lua's generic for loop.
- `iter.filter(filter_fn, iter_fn, state, init_ctrl_var, close)`: function - filter values in an iterator.
  - Arguments:
    - `filter_fn`: function - the filter test.  The resulting iterator will only produce values for which `filter_fn` returns true.
    - `iter_fn, state, init_ctrl_var, close`: iterator vars - variable sequence provided by `ipairs`, `pairs`, or any other source of values intended to be used with Lua's generic for loop.
- `iter.reduce(init_accum, reduce_fn, iter_fn, state, init_ctrl_var, close)`: function - reduce values produced by the iterator using a reduce function
  - Arguments:
    - `init_accum`: any - the initial accumulator value
    - `reduce_fn`: function - a function that takes as arguments the current accumulator value and the next value produced by the iterator, and returns the new accumulator value.
    - `iter_fn, state, init_ctrl_var, close`: iterator vars - variable sequence provided by `ipairs`, `pairs`, or any other source of values intended to be used with Lua's generic for loop.

iter object methods
- `map(map_fn)`: function - apply the map function and return a new iter object
- `enumerate()`: function - apply the enumerate function and return a new iter object
- `filter(filter_fn)`: function - apply the filter function and return a new iter object
- `reduce(init_accum, reduce_fn)`: function - apply the reduce function and return a new iter object
- `for_each(fn)`: function - iterate through the iter object's values and call `fn` for each item
- `to_table()`: function - return a list containing all the values produced by the iterator

Example usage:

```lua
local iter = require("iter")

local original_words = { "dais", "squirrel", "fort", "part" }
local new_words = iter(ipairs(original_words))
                    :filter(function(i, w) return w ~= "squirrel" end)
                    :map(function(i, w) return i, w.."y")
                    :to_table()
assert(new_words[1] == "daisy" and new_words[2] == "forty" and new_words[3] == "party")
```

#### Module `json`

Module for (de)serializing json values.  Value types are mapped using the following (json -> lua) mapping:

- `object` -> `table`
- `array` -> `table`
- `number` -> `float`
- `bool` -> `bool`
- `string` -> `string`
- `null` -> `nil`

Module contents:

- `json.load(path)`: function - open file at `path` and parse its contents as json, returning the parsed lua value
- `json.loads(json_str)`: function - parse the given string as json, returning the parsed lua value
- `json.dump(path, val)`: function - serialize the lua value `val` to json and write it to the file at `path`
- `json.dumps(val)`: function - serialize the lua value `val` to json and return the json string

#### Module `toml`

Module for (de)serializing toml values.  Value types are mapped using the following (toml -> lua) mapping:

- `table` -> `table`
- `array` -> `table`
- `integer` -> `integer`
- `float` -> `float`
- `bool` -> `bool`
- `string` -> `string`
- `datetime` -> `userdata` (which implements the `__tostring` metamethod, and can be serialized back to a toml `datetime`)

Module contents:

- `toml.load(path)`: function - open file at `path` and parse its contents as toml, returning the parsed lua value
- `toml.loads(toml_str)`: function - parse the given string as toml, returning the parsed lua value
- `toml.dump(path, val)`: function - serialize the lua value `val` to toml and write it to the file at `path`
- `toml.dumps(val)`: function - serialize the lua value `val` to toml and return the toml string

#### Module `maybe`

Object type for elegantly handling values that might be `nil`.  The maybe object implements nearly all metamethods, (it does not implement `__newindex`,) allowing for use with most operators.

- `maybe(val)`: function - create a maybe object wrapping `val`

`maybe` object methods:
- `and_then(fn)`: function - if this wrapped value is `nil`, return `maybe(nil)`, else return `maybe(fn(self.value))`
- `or_else(fn)`: function - if this wrapped value is `nil`, return `maybe(fn())`, else return `self`
- `value`: any - the value wrapped by this maybe object 

Example usage:

```lua
(maybe(nil) + 5).value -- nil
(maybe(5) + 5).value -- 10
(maybe({chapter_1={section_1="this is section 1"}})["chapter_1"]["section_1"]).value -- "this is section 1"
(maybe({chapter_1={section_1="this is section 1"}})["chapter_2"]["section_7"]).value -- nil
(maybe(nil)["chapter_1"]).value -- nil
(maybe("hello world"):and_then(function(v) return v:gsub("world", "universe") end)).value -- "hello universe"
(maybe(nil):and_then(function(v) return v:gsub("world", "universe") end)).value -- nil
(maybe(nil)
  :or_else(function () return "hello world" end)
  :and_then(function (v) return v:gsub("world", "universe") end)
).value -- "hello universe"
```

#### Module `scope`

Provides functionality for executing some logic when a scope is exited

- `scope.on_exit(fn)`: function - create an object that will execute `fn` when it goes out of scope

Example usage:

```lua
local scope = require("scope")

function ()
  local scoped = scope.on_exit(function() print("function complete") end)
  -- do some stuff
end -- prints "function complete" upon exiting the function
```

#### Module `script_dir`

Provides a function for getting the directory that contains the lua script file currently being run

- `script_dir()`: function - returns the directory that contains the lua script file currently being run

#### Module `version`

Provides logic for comparing version numbers.  A version object, created with the `version` constructor function, supports comparison operators `<`, `>`, `==`, `~=` to compare with other version objects or string representations of versions.

Version comparison should work for most dot-delimited version numbers.

- `version(version_str)`: function - creates a version object

#### Module `tblext`

Provides additional table manipulation functionality on top of Lua's `table` module.  Unlike the `table`module, `tblext` is intended for use with tables both used as sequences or maps.

- `tblext.extend(target, source, [start_index])`: function - merge properties from `source` into `target`.  If a key exists in both `source` and `target`, the value from `source` overwrites the value in `target`. Integer keys behave differently from other keys.  Integer keys are offset by `start_index-1` and then merged.  The default value for `start_index` is `#target+1`, meaning sequence values in `source` will be appended to the existing sequence values in `target`.  If you'd like sequence values in `source` to be merged into `target` just like any other key type, pass in `1` for `start_index`.
- `tblext.format(value)`: function - Returns a string representation for provided table `value`

## License

This project, with the exception of the `examples` directory, is licensed under the GPLv3.0 license.  See [COPYING](https://github.com/jdarais/cobble/blob/main/COPYING).  The contents of the `examples` directory are free to use without restrictions.

This project includes libraries licensed under the [MIT License](https://github.com/jdarais/cobble/blob/main/licenses/MIT.txt).

LMDB is licened under the [OpenLDAP Public License](https://github.com/jdarais/cobble/blob/main/licenses/OpenLDAP.txt)
