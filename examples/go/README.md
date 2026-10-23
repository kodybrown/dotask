# Go tasks

With the complete prebuilt dotask distribution and an installed Go 1.22+
compiler, run from this directory:

```sh
dotask help hello
dotask hello --lang go --name 'Go developer'
```

Expected output: `Hello, Go developer!`. Help works without Go. Execution uses
only the installed Go compiler, the bundled SDK, and declared support files.
No Rust/Cargo or .NET is needed. No consumer Go module or workspace is required.

Import `dotask.local/sdk` as `dotask`. Local packages beside the entry file use the
`dotask.local/task/` prefix; declare each support file in the metadata header.
Only the selected entry file is compiled, so other tasks can have their own main.
Generated manifests and executables stay in dotask's external cache.

See [target authoring](../../docs/TARGETS.md#go-tasks) for the SDK and isolation
contract. Dedicated installer acceptance fixtures live in
`src/dotask-cli/tests/go_tasks.rs`; they use the shared native creator and
prebuilt doinstall tools without modifying a real consumer project.
