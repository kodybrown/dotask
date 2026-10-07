// dotask: 1
// description: Create a 7z archive containing the named source directory.
// options:
//   - { name: source, type: path, required: true }
//   - { name: output, type: path, required: true }
//   - { name: archiver, default: 7z, description: 7-Zip executable name or path. }
// requires: [{ kind: file, value: archive/_support/Archive.rs }]
// end-dotask
#[path = "_support/Archive.rs"]
mod archive;
fn main() {
  dotask_sdk::run(|project| archive::create(project, "7z"));
}
