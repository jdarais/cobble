# Workspace Definition

## cobble.toml

The presence of a `cobble.toml` file marks the root of a Cobble workspace.  The `cobble.toml` file can also contain configuration for the workspace.  The following options can be configured in the `cobble.toml` file:

- `root_projects`: _array[string]_ - A list of root project paths to include in the workspace (Default = `["."]`)
- `num_threads`: _int_ - Number of threads to use for executing tasks (Default = `5`)
- `stdout`: _"always" | "never" | "on_fail"_ - When to display stdout output from tasks (Default = `"on_fail"`)
- `stderr`: _"always" | "never" | "on_fail"_ - When to display stderr output from tasks (Default = `"on_fail"`)
- `output`: _"always" | "never" | "on_fail"_ - Sets both `stdout` and `stderr`.  If `stdout` or `stderr` properties are present, they will take precedence over `output`.
- `vars`: _table_ - Variables which can be used in actions

## Example

An example of what a `cobble.toml` file might look like:

```toml
root_projects = [ "./project_a", "./project_b" ]
num_threads = 10

[vars]
foo = "bar"
python.version = "3.11"
```

