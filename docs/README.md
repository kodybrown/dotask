# dotask documentation

Start with the [repository README](../README.md) to build the local preview and
run your first example. The preview is not yet published to a public package feed.

| Goal                                                  | Read                                                             |
| ----------------------------------------------------- | ---------------------------------------------------------------- |
| Build and verify the source checkout                 | [Build instructions](../README.md#build-and-verify)               |
| Publish native releases and consume pinned bundles | [Release procedure](RELEASING.md) |
| Install after building                               | [Install dotask](../README.md#install-dotask)                     |
| Initialize a project                                 | [Project setup](USAGE.md#initialize-a-project)                    |
| Install applications, choose directories, or uninstall | [Application installers](INSTALLATION.md) |
| Try an example without installing                    | [Source preview](../README.md#try-the-example-without-installing) |
| Run tasks and understand CLI behavior                 | [Usage guide](USAGE.md)                                          |
| Download, share, update, or remove tasks safely       | [Shared tasks](SHARED-TASKS.md)                                  |
| Reuse Rust build/run/test and explicit version tasks | [Rust console-tool group](../shared-tasks/rust/README.md) |
| Create a group interactively | [Creation wizard](TARGETS.md#interactive-group-creation) |
| Compose existing tasks without C# | [YAML task groups](TARGETS.md#yaml-task-groups) |
| Read/render configuration and run native installation workflows | [Native YAML workflow actions](YAML-WORKFLOWS.md) |
| Fetch JSON and refresh authored Markdown project pages | [GitHub project-page example](../examples/github-projects/README.md) |
| Create a task or look up library APIs                 | [Authoring and configuration reference](TARGETS.md)              |
| Write a Go task with compiler-free metadata | [Go tasks and SDK](TARGETS.md#go-tasks) |
| Write a Rust task with compiler-free metadata | [Rust tasks and SDK](TARGETS.md#rust-tasks) |
| Ask an AI assistant to create or maintain tasks       | [AI assistant guide and reusable instructions](AI-ASSISTANTS.md) |
| Follow a runnable example with expected output        | [Basic example](../examples/basic/README.md)                     |
| Understand implementation decisions                   | [Design and execution contracts](DESIGN.md)                      |
| Develop the native CLI with the C# behavior reference | [Preview commands](../README.md#rust-cli-development-preview) and [transition design](DESIGN.md#rust-cli-transition) |
| Continue Rust acceptance, packaging, and task authoring | [Rust continuation runbook](RUST-CONTINUATION.md) |
| Compare measured Windows CLI deployment sizes | [Baseline AOT and Rust size experiment](scratch/2026-10-05-windows-cli-size-experiment.md) |
| Review external installer integration boundaries | [External installer integration](DESIGN.md#external-installer-integration) |
| Check tested hosts, shells, and remaining limitations | [Verification status](VERIFICATION.md)                           |
| Develop dotask itself                                 | [Repository agent instructions](../AGENTS.md)                    |
| Review approved completed changes                     | [Changelog](CHANGELOG.md)                                        |
| Read the project license                              | [MIT License](../LICENSE.md)                                     |

The usage guide owns the command-line contract, the authoring reference owns the
task/configuration/library contract, and the design document explains execution
internals. The AI guide provides a workflow using those contracts. Keep these
references aligned when behavior changes.
