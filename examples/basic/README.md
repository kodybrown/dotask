# Basic dotask example

This example needs the .NET 10 SDK required by dotask and a built or installed
CLI. It demonstrates metadata, YAML defaults, typed settings, nested calls, YAML task groups, and
writing a project-relative file. It does not need a separate sample application.

| File                               | Purpose                                                                               |
| ---------------------------------- | ------------------------------------------------------------------------------------- |
| [.dotasks.yaml](.dotasks.yaml)     | Project name/description, shared greeting/output settings, and `hello`'s name default |
| [.tasks/hello.cs](.tasks/hello.cs) | Greeting with `--name`/`-n` and `--configuration`/`-c`                                |
| [.tasks/check.cs](.tasks/check.cs) | Require the `message` setting and print the project root                              |
| [.tasks/greet.task](.tasks/greet.task) | Check configuration, greet with explicit parameters, and skip an absent optional task |
| [.tasks/write.cs](.tasks/write.cs) | Call `check` and `hello`, then write the output file                                  |

## Run from the dotask checkout root

With `dotask` on PATH:

```text
dotask --use-dir examples/basic/.tasks
dotask --help
dotask --use-dir examples/basic/.tasks help hello
dotask --use-dir examples/basic/.tasks hello
dotask --use-dir examples/basic/.tasks hello -n "Ada Lovelace" configuration=release
dotask --use-dir examples/basic/.tasks write
```

With `--verbose`, the project summary should start with `dotask example` and its description, then
show `tasks: ./.tasks`, the `message`/`output` settings, `check`/`hello`/`write`/`greet`
targets, and target options. `dotask help --verbose` shows the same summary when run in
`examples/basic`.
`dotask --help` shows CLI usage only, without those project details.
`hello`'s help should show
the effective name default `Developer` from YAML, overriding `World` in its XML.
Running `hello` with no arguments prints:

```text
Hello from dotask, Developer!
Host: Linux; configuration: Debug
```

The host line uses `Windows` or `MacOS` on those hosts. With the explicit
arguments above, expect `Hello from dotask, Ada Lovelace!` and configuration
`Release` (the declared choice spelling).

`write` prints a configuration check, greets `Nested target` with configuration
`Release`, and creates or replaces `examples/basic/artifacts/example.txt` with
`Written by dotask.` followed by a newline. Its printed absolute path is rooted
at `examples/basic`, because its root `.dotasks.yaml` anchors the selected `.tasks`.
Running it again repeats the calls and replaces the same output file.

To demonstrate validation without running `hello`'s entry point:

```text
dotask --use-dir examples/basic/.tasks hello --configuration invalid
```

Expect exit code 1 and a message saying to choose `Debug, Release`.

## Run without installing dotask

From the checkout root:

```text
dotnet build dotask.slnx -c Release
dotnet run --project src/Dotask.Cli -c Release --no-build -- --use-dir examples/basic/.tasks hello
dotnet run --project src/Dotask.Cli -c Release --no-build -- --use-dir examples/basic/.tasks write
```

When using an installed CLI, you can instead change into `examples/basic` and
run `dotask hello`; normal upward discovery finds its `.tasks` directory.

Continue with [task authoring](../../docs/TARGETS.md),
[completion and troubleshooting](../../docs/USAGE.md), or the
[AI assistant guide](../../docs/AI-ASSISTANTS.md).

## Run the YAML group

From the checkout root, without installing:

```sh
./build.sh --use-dir examples/basic/.tasks help greet
./build.sh --use-dir examples/basic/.tasks greet
```

`greet` first prints the configuration check, then `Hello from dotask, YAML group!`
and `Host: Linux; configuration: Release` (the host varies by operating system).
The absent `optional-check` is silently skipped. See
[YAML task groups](../../docs/TARGETS.md#yaml-task-groups) for the schema.
