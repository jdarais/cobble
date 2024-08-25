# cobl CLI

More detailed documentation of the `cobl` CLI coming soon.

For now, here is the output of `cobl --help`:

```
Commands:
  list   List available tasks
  run    Run tasks
  clean  Clean tasks
  tool   Interact with tools defined in the workspace
  env    Interact with build environments defined in the workspace
  help   Print this message or the help of the given subcommand(s)

Options:
  -n, --num-threads <N>
          The number of threads to use for running tasks. (Default: 5)
  -v, --var <VAR=VALUE>
          Set the value for a variable
      --task-output <always|never|on_fail>

      --task-stdout <always|never|on_fail>

      --task-stderr <always|never|on_fail>

      --version
          Display the version of this application and exit
  -h, --help
          Print help
```
