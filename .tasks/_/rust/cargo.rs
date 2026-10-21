// dotask: 1
// description: Reusable Cargo operations with external outputs and scanned build executables.
// options:
//   - {name: operation, required: true, choices: [build, run, test, fmt, clippy]}
//   - {name: manifest, type: path, default: Cargo.toml}
//   - {name: package, default: ''}
//   - {name: profile, default: release}
//   - {name: bin, default: ''}
//   - {name: args, default: '[]', description: JSON array of literal application arguments.}
//   - {name: check, type: bool, default: false}
//   - {name: format-tasks, type: bool, default: false}
// requires: [{kind: tool, value: cargo}, {kind: file, value: rust/_support/Rust.rs}, {kind: file, value: rust/README.md}]
// end-dotask
#[path = "_support/Rust.rs"]
mod rust;
fn main() {
  dotask_sdk::run(rust::operation);
}
