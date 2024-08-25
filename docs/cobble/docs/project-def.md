# Project Definition

## Intro

Project definition files are Lua scripts in which tasks, build environments, and tools are defined.  When Cobble executes the project definition files, the functions for defining these project building blocks are available as globals in the lua environment.

## Project Definition Functions

### task

_function_ - Define a task

`task(task_def)`

##### Arguments

- `task_def`: _table_ - task definition properties
    - `name`: _string_ - the task name
    - `actions`: _table_ - A list of `action`s that define the execution logic for the task.
    - `default`: _bool | nil_ - whether the task is a default task for the project.  When `cobl run` is given a project name, the default tasks for that project are run.  If no tasks are defined as default for a project, passing the project name to `cobl run` runs all tasks in the project. (default=false)
    - `always_run`: _bool | nil_ - If true, the task will always be run if selected, regardless of whether its dependencies and artifacts are up-to-date. (default=false)
    - `interactive`: _bool | nil_ - If true, child processes launched by this task can attach to stdin.  Note that interactive tasks cannot run in parallel. (default=false)
    - `stdout`: _"always" | "never" | "on_fail" | nil_ - When to display stdout output from the task (default="on_fail")
    - `stderr`: _"always" | "never" | "on_fail" | nil_ - When to display stderr output from the task (default="on_fail")
    - `output`: _"always" | "never" | "on_fail" | nil_ - Setting this property will set both `stdout` and `stderr` properties.  If either `stdout` or `stderr` properties are present, they will take precedence over the value provided by `output`.
    - `env`: _string | table | nil_ - If provided, the named action environment is available to all actions in the task.  A table mapping an environment alias to an environment name is also valid, however only a single environment can be specified at the task level.
    - `clean`: _table | nil_ - A list of `action`s to run when the task is selected in a `cobl clean` command.
    - `deps`: _table | nil_ - A mapping of dependency type to a list of dependencies
        - `files`: _table | nil_ - A list of file dependency paths
        - `tasks`: _table | nil_ - A list of task dependency names
        - `calc`: _table | nil_ - A list of tasks to execute for calculating dependencies.  The calc task's output, (i.e. the return value of the tasks last action,) should match the same structure as the `deps` property for task definitions, with the exception that calc dependencies producing additional calc dependencies is not supported.  Calculated results will be combined and added to the statically declared dependencies.
    - `artifacts`: _table | nil_ - A mapping of artifact type to a list of artifacts
        - `files`: _table | nil_ - A list of file artifact paths
        - `calc`: _table | nil_ - A list of tasks to execute for calculating artifacts.  The calc task's output should be a list of file paths.

##### Returns

_nil_

### env

 _function_ - Define a build environment

`env(env_def)`

##### Arguments

- `env_def`: _table_ - Build environment definition properties
    - `name`: _string_ - The build environment name
    - `setup_task`: *task_def* - The task to execute to set up the build environment, (e.g. "npm install").  All `task_def` properties are supported except for `name`.  The setup task will be given the same name as the build environment.
    - `action`: *action_def* - An action that will run a command in the build environment, (e.g. "npm exec").  For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.

##### Returns

_nil_

### tool

_function_ - Define an external tool

`tool(tool_def)`

##### Arguments

- `tool_def`: _table_ - Tool definition properties
    - `name`: _string_ - The tool name.  Unlike tasks and build environments, tool names are global, and are not combined with a project name to create a full name.
    - `check`: *action_def* - An `action` for checking whether the tool is correctly installed, (correct version, etc.).  If the check fails, the check action should call `error` to raise an error.
    - `action`: *action_def* - An `action` that will execute the tool. For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.

##### Returns

_nil_

### project_dir

_function_ - Add a project in a subdirectory

`project_dir(path)`

##### Arguments

- `path`: string - The path to the project

##### Returns

_nil_

### project

_function_ - Add a named project in the same directory as the current project

`project(name, project_def_cb)`

##### Arguments

- `name`: _string_ - The name of the subproject
- `project_def_cb`: _function_ - A function that, when called, creates `task`, `env`, and `tool` definitions for the subproject.

## Built-in Cmd Tool

Cobble provides one built-in tool: the `cmd` tool.  The `cmd` tool uses Cobble's built-in `cmd` module to execute a command in a subprocess, passing in the action context's `project.dir`, `out`, and `err` properties as the `cwd`, `out`, and `err` arguments to the `cmd` function, respectively.

## Actions

Actions can be defined using a table or function.  See details on the different ways to define an action below.

### Arg-list Actions

An arg-list action is defined using a Lua table with an optional `tool` or `env` property defining the tool or action environment that should be used to execute the action.  (If omitted, the `cmd` tool is used.)

The remaining table entries are passed to the referenced tool or environment as arguments.

If the action itself receives arguments, (e.g. if the action is defined in a tool or action environment,) those arguments are appended to the arguments defined by the action before passing them to the referenced tool or environment.  This allows easy definition of tools or environments that simply accept arguments and append them to a particular command to be executed.

##### Example

```lua
tool {
  name = "npm",
  -- Action that runs "npm <args>" when the tool is invoked.
  -- Same as { tool = "cmd", "npm" }
  action = { "npm" }
}

env {
  name = "npm_env",
  setup_task = {
    actions = {
      -- Action that runs "npm install" using the tool defined above
      { tool = "npm", "install" }
    }
  },
  -- Action that runs "npm exec -- <args>" when the env is invoked
  action = { tool = "npm", "exec", "--" }
}

task {
  name = "lint",
  actions = {
    -- Action that runs "npm exec -- eslint src/" using the env defined above
    { env = "npm_env", "eslint", "src/" }
  }
}
```

### Action Functions

Action functions are defined using a Lua function.  The function is passed in a single argument: an "action context", which provides useful information and functionality to the action.

An action function can be defined as a standalone function, or in a table alongside tool or action environment references.  Unlike arg-list actions, action functions can have any number of tool or environment references, which will then make those tools and environments available in the action context passed to the action.

##### Examples

```lua
local tblext = require("tblext")

tool {
  name = "npm",
  -- Action that runs "npm <args>" when the tool is invoked
  -- The Cobble built-in module "tblext" is used to append the passed-in arguments to the { "npm" } table
  action = function (c)
    return c.tool.cmd { tblext.extend({ "npm" }, c.args) }
  end
}

env {
  name = "npm_env",
  setup_task = {
    actions = {
      -- Action that runs "npm install" using the tool defined above
      {
        tool = "npm",
        function (c) return c.tool.npm { "install" } end
      }
    }
  },
  -- Action that runs "npm exec -- <args>" when the env is invoked
  action = {
    tool = "npm"
    function (c) return c.tool.npm { tblext.extend({ "exec", "--" }, c.args) end
  }
}

task {
  name = "lint",
  actions = {
    -- Action that runs "npm exec -- eslint src/" using the env defined above
    {
      env = "npm_env",
      function (c) return c.env.npm_env { "eslint", "src/" } end
    }
  }
}
```
#### Action Context

The action context passed to action functions has the following properties:

- `tool`: _table_ - A table mapping tool names to tool invocation functions. Contains an entry for each tool available to the action
- `env`: _table_ - A table mapping build environment names to invocation functions. Contains an entry for each build environment available to the action
- `action`: _action_ - A copy of the action definition that defines the current action
- `files`: _table_ - A table containing file dependencies of the task that the action belongs to.  Table keys are file paths as they are declared in the task.  Each value is a table containing a `path` and `hash` property, which provides both the path of the file relative to the workspace root and the fingerprint hash of the file contents.
- `tasks`: _table_ - A table containing information about task dependencies of the task that the action belongs to.  Table keys are task dependency names as they were declared in the action's task.  Each value is the output returned by the last action in the task dependency.
- `vars`: _table_ - A table containing information about variable dependencies of the task that the action belongs to.  Table keys are variable names as they were declared in the action's task.   Each value is the value of that variable.
- `project`: _table_ - A table containing information about the project in which the currently executing task is defined.
    - `dir`: _string_ - The project directory in which the currently executing task is defined.
- `args`: A table containing arguments passed to the action.  If the action is part of a tool or action environment being invoked, `args` contains the arguments passed to the tool or environment.  If the action belongs to a task, `args` contains the return value of the previous action executed in the same task, if there is one.
- `out`: A function to send text to stdout.  (Note that this function is preferred over the Lua `print` function, since it manages buffering and ensuring that a task's output gets printed out together in the console, instead of being interleaved with the output of other tasks being run in parallel.)
- `err`: A function to send text to stderr.  (Note that this function is preferred over the Lua `print` function for the same reasons as with the `out` function.)

#### Action Execution

When Cobble executes tasks, it distributes tasks among multiple threads, each with their own Lua environment.  This presents a challenge for actions funcitons: the action function implementation must be copied from the Lua environment in which the action was defined into the Lua environment responsible for executing the task.  To accomplish this, action functions are extracted from the Lua environment into an in-memory representation, along with any external local variables referenced by the function, (i.e "upvalues").  No global variables are extracted.  This results in a few limitations that are not present in a typical Lua environment:

##### Module References

Upvalue references to modules in an action function can cause large amounts of code to be extracted along with the action function itself.  It is recommended to declare separate local variables for the individual module members that are used by the action function if they are to be referenced as upvalues.  Alternatively, you can `require` the module from within the action function.  For example:

```lua
local mymod = require("my.large.module")

task {
  name = "bad_example",
  actions = {
    -- This action will cause the entire "mymod" module to
    -- be extracted along with the action!
    function (c) mymod.dowork() end
  }
}

local dowork = mymod.dowork

task {
  name = "better_example",
  actions = {
    -- This will only require mymod's "dowork" function to
    -- be extracted along with the action
    function (c) dowork() end
  }
}

task {
  name = "best_example",
  actions = {
    -- This function has no upvalues to be extracted.
    function (c)
      local m = require("my.large.module")
      m.dowork()
    end
  }
}
```

Additionally, any native module references other than those to Cobble's built-in modules will cause action extraction to fail.


