# Project Definition

## Intro

Project definition files are Lua scripts in which tasks, build environments, and tools are defined.  When Cobble executes the project definition files, the functions for defining these project building blocks are available as globals in the lua environment.

## Project Definition Functions

### task

_function_ - Define a task

`task(task_def)`

<h5>Arguments</h5>

- `task_def`: _table_ - task definition properties
    - `name`: _string_ - the task name
    - `actions`: _table_ - A list of `action`s that define the execution logic for the task.
    - `default`: _bool | nil_ - whether the task is a default task for the project.  When `cobl run` is given a project name, the default tasks for that project are run.  If no tasks are defined as default for a project, passing the project name to `cobl run` runs all tasks in the project. (default=false)
    - `always_run`: _bool | nil_ - If true, the task will always be run if selected, regardless of whether its dependencies and artifacts are up-to-date. (default=false)
    - `interactive`: _bool | nil_ - If true, child processes launched by this task can attach to stdin.  Note that interactive tasks cannot run in parallel. (default=false)
    - `stdout`: _"always" | "never" | "on_fail" | nil_ - When to display stdout output from the task (default="on_fail")
    - `stderr`: _"always" | "never" | "on_fail" | nil_ - When to display stderr output from the task (default="on_fail")
    - `output`: _"always" | "never" | "on_fail" | nil_ - Setting this property will set both `stdout` and `stderr` properties.  If either `stdout` or `stderr` properties are present, they will take precedence over the value provided by `output`.
    - `env`: _string | table | nil_ - If provided, the named environment is available to all actions in the task.  A table mapping an environment alias to an environment name is also valid, however only a single environment can be specified at the task level.
    - `clean`: _table | nil_ - A list of `action`s to run when the task is selected in a `cobl clean` command.
    - `deps`: _table | nil_ - A mapping of dependency type to a list of dependencies
        - `files`: _table | nil_ - A list of file dependency paths
        - `tasks`: _table | nil_ - A list of task dependency names
        - `calc`: _table | nil_ - A list of tasks to execute for calculating dependencies.  The calc task's output, (i.e. the return value of the tasks last action,) should match the same structure as the `deps` property for task definitions, with the exception that calc dependencies producing additional calc dependencies is not supported.  Calculated results will be combined and added to the statically declared dependencies.
    - `artifacts`: _table | nil_ - A mapping of artifact type to a list of artifacts
        - `files`: _table | nil_ - A list of file artifact paths
        - `calc`: _table | nil_ - A list of tasks to execute for calculating artifacts.  The calc task's output should be a list of file paths.

<h5>Returns</h5>

_nil_

### env

 _function_ - Define a build environment

`env(env_def)`

<h5>Arguments</h5>

- `env_def`: _table_ - Build environment definition properties
    - `name`: _string_ - The build environment name
    - `setup_task`: *task_def* - The task to execute to set up the build environment, (e.g. "npm install").  All `task_def` properties are supported except for `name`.  The setup task will be given the same name as the build environment.
    - `action`: *action_def* - An action that will run a command in the build environment, (e.g. "npm exec").  For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.

<h5>Returns</h5>

_nil_

### tool

_function_ - Define an external tool

`tool(tool_def)`

<h5>Arguments</h5>

- `tool_def`: _table_ - Tool definition properties
    - `name`: _string_ - The tool name.  Unlike tasks and build environments, tool names are global, and are not combined with a project name to create a full name.
    - `check`: *action_def* - An `action` for checking whether the tool is correctly installed, (correct version, etc.).  If the check fails, the check action should call `error` to raise an error.
    - `action`: *action_def* - An `action` that will execute the tool. For function actions, the arguments passed to the action are available in `c.args`.  For actions defined using a table, the args are appended to the table and passed to the tool or build environment referenced by the action.

<h5>Returns</h5>

_nil_

### project_dir

_function_ - Add a project in a subdirectory

`project_dir(path)`

<h5>Arguments</h5>

- `path`: string - The path to the project

<h5>Returns</h5>

_nil_

### project

_function_ - Add a named project in the same directory as the current project

`project(name, project_def_cb)`

<h5>Arguments</h5>

- `name`: _string_ - The name of the subproject
- `project_def_cb`: _function_ - A function that, when called, creates `task`, `env`, and `tool` definitions for the subproject.

## Actions

