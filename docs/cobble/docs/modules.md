# Built-in Modules

## Globals

The following global variables are available in any lua environment:

- `WORKSPACE`: _table_ - Contains information about the workspace
    - `dir`: _string_ - Absolute path to the workspace directory
- `PLATFORM`: _table_ - Contains information about the platform Cobble is able to discover
    - `arch`: _string_ - the platform architecture. One of [std::env::consts::ARCH](https://doc.rust-lang.org/std/env/consts/index.html)
    - `os_family`: _string_ - the platform OS family. One of [std::env::consts::FAMILY](https://doc.rust-lang.org/std/env/consts/constant.FAMILY.html)
    - `os`: _string_ - the platform OS. One of [std::env::consts::OS](https://doc.rust-lang.org/std/env/consts/constant.OS.html)

## Cobble Modules

### cmd

The `cmd` module is a function that invokes a command, providing some additional functionality over Lua's `os.execute` function, as well as integration with Cobble actions.

Note that the `cmd` module is different from the built-in `cmd` tool.  The `cmd` module will use the workspace directory as CWD by default, not any project directory.

#### cmd

_function_ - Execute a command

`cmd(args)`

##### Arguments

- `args`: _table_
    - `cwd`: _string | nil_ - Current working directory to run the command with
    - `out`: _string | nil_ - Callback to be called with any stdout output
    - `err`: _string | nil_ - Callback to be called with any stderr output
    - `...` _(sequence values)_: _string_ - Any positional (numeric index) table elements are interpreted as the command and command args to execute

##### Returns

- _table_ - Status and output of the launched process
    - `status`: _int_ - The return status of the launched process
    - `stdout`: _string_ - The stdout output of the process
    - `stderr`: _string_ - The stderr output of the process

### path

#### path.SEP

_string_ - The path separator character for the current OS

#### path.glob

_function_ - Get files matching a pattern in a directory tree

`path.glob([base], pattern)`

##### Arguments

- `base` _(optional)_: _string_ - Base path to search from.  Returned file paths are relative to the base path.  (Default: CWD)
- `pattern`: _string_ - Pattern to match files with.  Can include `*` or `**` wildcards.

##### Returns

- _table_ - A list of paths found that match the given pattern
    - `...` _(sequence values)_: _string_


#### path.join

_function_ - Join path segments using the OS-specific path separator

`path.join(...)`

##### Arguments

- `...`: _string_ - path segments to join

##### Returns

- _string_ - the joined path


#### path.is_dir

_function_ - Test whether a path exists and is a directory

`path.is_dir(path)`

##### Arguments

- `path`: _string_ - The path to test

##### Returns

- _boolean_ - True if the path exists and is a directory.  False otherwise.


#### path.is_file

_function_ - Test whether a path exists and is a file

`path.is_file(path)`

##### Arguments

- `path`: _string_ - The path to test

##### Returns

- _boolean_ - True if the path exists and is a file.  False otherwise.

### iter

The `iter` module provides a convenient, functional interface for manipulating lists lazily and efficiently.

The `iter` constructor function and module-level functions are intended to be used with Lua's `ipairs` or `pairs` functions, or any set of values intended to used with Lua's [generic for](https://www.lua.org/manual/5.4/manual.html#3.3.5) loop. For example:

```lua
local iter = require("iter")

local original_words = { "dais", "squirrel", "fort", "part" }
local new_words = iter(ipairs(original_words))
                    :filter(function(i, w) return w ~= "squirrel" end)
                    :map(function(i, w) return i, w.."y")
                    :to_table()
assert(
    new_words[1] == "daisy" and
    new_words[2] == "forty" and
    new_words[3] == "party"
)
```

#### iter

_function_ - Wrap a set of iterator functions in an `iter` object.

`iter(iter_fn, state, init_val, close)`

##### Arguments

- `iter_fn`: _function_ - Iterator function
- `state`: _any_ - State to be passed to the iterator function at each iteration
- `init_val`: _any_ - Initial value for the control variable
- `close`: _to-be-closed_ - Variable that will be closed when the loop ends

##### Returns

- _iter_ - An `iter` object

### iter object

#### iter:map

_function_ - Apply a map operation to the iterator

`iter_obj:map(map_fn)`

##### Arguments

- `map_fn`: _function_ - A function that takes in the value(s) produced by an iteration of the iterator and returns new values.

##### Returns

_iter_ - A new `iter` object that produces values that are the result of applying `map_fn` to the original iterator's values

#### iter:reduce

_function_ - Apply a reduce operation to the iterator

`iter_obj:reduce(init, reduce_fn)`

##### Arguments

- `init`: _any_ - Initial value to use as the accumulator
- `reduce_fn`: _function_ - A function that takes the accumulator value as its first argument, followed by the value(s) produced by an iteration of the iterator, and returns a new accumulator value to be used with the next iteration.

##### Returns

_any_ - The accumulator value returned by the `reduce_fn` call on the last iteration

#### iter:filter

_function_ - Apply a filter operation to the iterator

`iter_obj:filter(filter_fn)`

##### Arguments

- `filter_fn`: _function_ - A function that takes the value(s) produced by an iteration of the iterator, and returns a boolean value expressing whether or not the value(s) should be included in the resulting iterator.

##### Returns

_iter_ - An `iter` object that produces all values produced by the original iterator for which `filter_fn` returns `true`

#### iter:enumerate

_function_ - Append an iteration count to the beginning of each set of values produced by the iterator

`iter_obj:enumerate()`

##### Returns

_iter_ - An `iter` object that produces the same values as the original iterator, but with a counter value appended to the beginning, starting at 1 for the first iteration

#### iter:for_each

_function_ - Execute a function for each value or set of values produced by the iterator

`iter_obj:for_each(fn)`

##### Arguments

- `fn`: _function_ - A function that takes in the value(s) produced by an iteration of the iterator

##### Returns

_nil_

#### iter:to_table

_function_ - Iterate over the iterator and collect the values into a table.  The iterator is expected to produce two values for each iteration: a key and a value.  This is the structure of values produced by `ipairs` and `pairs`, which produce a key and value pair on each iteration.

##### Returns

_table_ - The table into which the iterator values were collected


### json

Module for (de)serializing json values.  When converting between Lua and json types:

- JSON numbers are always converted to Lua floats, regardless of whether or not they contain integral values
- If a Lua table contains consecutive integer keys starting from 1, it is converted to a json array.  Otherwise, it is converted to a json object.

#### json.load

_function_ - Read and parse json from a file

`json.load(path)`

##### Arguments

- `path`: _string_ - Path to the json file to read

##### Returns

- _any_ - The json data converted to a Lua value

#### json.loads

_function_ - Parse a json string

`json.load(s)`

##### Arguments

- `s`: _string_ - The json string to parse

##### Returns

_any_ - The json data converted to a Lua value

#### json.dump

_function_ - Convert a Lua value to json and write it to a file

`json.dump(path, val)`

##### Arguments

- `path`: _string_ - Path to the file where the json data should be written
- `val`: _any_ - The Lua value to serialize to json

##### Returns

_nil_

#### json.dumps

_function_ - Convert a Lua value to a json string

`json.dumps(val)`

##### Arguments

- `val`: _any_ - The Lua value to serialize to json

##### Returns

_string_ - The serialized json value

### toml

Module for (de)serializing toml values.  When converting between Lua and toml types:

- TOML datetime values are converted to a Lua userdata, which can be converted to a string or serialized back to a toml datetime value.
- If a Lua table contains consecutive integer keys starting at 1, it is converted into a toml array.  Otherwise, it is converted to a toml table.

#### toml.load

_function_ - Read and parse toml from a file

`toml.load(path)`

##### Arguments

- `path`: _string_ - Path to the toml file to read

##### Returns

_any_ - The toml data converted to a Lua value

#### toml.loads

_function_ - Parse a toml string

`toml.load(s)`

##### Arguments

- `s`: _string_ - The toml string to parse

##### Returns

_any_ - The toml data converted to a Lua value

#### toml.dump

_function_ - Convert a Lua value to toml and write it to a file

`toml.dump(path, val)`

##### Arguments

- `path`: _string_ - Path to the file where the toml data should be written
- `val`: _any_ - The Lua value to serialize to toml

##### Returns

_nil_

#### toml.dumps

_function_ - Convert a Lua value to a toml string

`toml.dumps(val)`

##### Arguments

- `val`: _any_ - The Lua value to serialize to toml

##### Returns

_string_ - The serialized toml value


### maybe

Object type for elegantly handling values that might be `nil`.  The maybe object implements nearly all metamethods, (it does not implement `__newindex`,) allowing for use with most operators.

Maybe objects are particularly useful for accessing values in nested data structures without having to check for `nil` at every level.

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

#### maybe

_function_ - Create a `maybe` object

`maybe(val)`

##### Arguments

- `val`: _any_ - The value to wrap in a `maybe` object

##### Returns

- _maybe_ - A maybe object

### maybe object

#### maybe.value

_any_ - The wrapped value held by the maybe object

#### maybe:and_then

_function_ - Perform an operation on the wrapped value if the wrapped value is non-nil

`maybe_obj:and_then(fn)`

##### Arguments

- `fn`: _function_ - A function that will be called with the wrapped value if it is a non-nil value

##### Returns

- _maybe_ - If the current wrapped value is `nil`, then `maybe(nil)` is returned, otherwise `maybe(fn(self.value))` is returned

#### maybe:or_else

_function_ - Call a function to provide a value if the wrapped value is nil

`maybe_obj:or_else(fn)`

##### Arguments

- `fn`: _function_ - A function that takes no arguments, and returns a value to be used if the current wrapped value is nil

##### Returns

_maybe_ - If the current wrapped value is `nil`, then `maybe(fn())` is returned, otherwise `self` is returned

### scope

Provides functionality for executing some logic when a scope is exited

#### scope.on_exit

_function_ - Execute some logic on scope exit

`scope.on_exit(fn)`

##### Arguments

- `fn`: _function_ - The function to exit when the returned object goes out of scope

##### Returns

- _any_ - A [to-be-closed](https://www.lua.org/manual/5.4/manual.html#3.3.8) variable that will execute `fn` when it goes out of scope

##### Example

```lua
local scope = require("scope")

function ()
  local scoped = scope.on_exit(function() print("function complete") end)
  -- do some stuff
end -- prints "function complete" upon exiting the function
```

### script_dir

#### script_dir

_function_ - returns the directory that contains the lua script file currently being run

`script_dir()`

##### Returns

_string_ - Path of the directory containing the lua script file currently being executed

### version

Provides logic for comparing version numbers.  A version object, created with the `version` constructor function, supports comparison operators `<`, `>`, `==`, `~=` to compare with other version objects or string representations of versions.

Version comparison should work for most dot-delimited version numbers.

#### version

_function_ - Create a `version` object

`version(version_str)`

##### Arguments

`version_str`: _string_ - A string representation of a version number

##### Returns

_version_ - A `version` object

### tblext

Provides additional table manipulation functionality on top of Lua's `table` module.  Unlike the `table`module, `tblext` is intended for use with tables both used as sequences or maps.

#### tblext.extend

_function_ - Merge entries from one table into another

If a key exists in both `source` and `target`, the value from `source` overwrites the value in `target`. Integer keys behave differently from other keys.  Integer keys are offset by `start_index-1` and then merged.  The default value for `start_index` is `#target+1`, meaning sequence values in `source` will be appended to the existing sequence values in `target`.  If you'd like sequence values in `source` to be merged into `target` just like any other key type, pass in `1` for `start_index`.

`tblext.extend(target, source, [start_index])`

##### Arguments

- `target`: _table_ - The table into which entries will be merged
- `source`: _table_ - The source table for entries to be merged from
- `start_index` _(optional)_: _int_ - The index at which to start appending values with integer keys.

##### Returns

_table_ - This function returns the table that was passed in as `target`

#### tblext.format

_function_ - Create a string representation of a table value that includes all of the key/value pairs in the table.  Table keys and values will also be formatted with `tblext.format`

`tblext.format(value)`

##### Arguments

- `value`: _table_ - The table value to format

##### Returns

_string_ - A string representation of the table
