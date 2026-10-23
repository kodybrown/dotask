// dotask: 1
// description: Test the bundled Go SDK and check formatting without consumer output.
// options: [{name: doinstall-tools, completion: directory, default: ""}]
// requires: [{kind: tool, value: go}, {kind: tool, value: gofmt}, {kind: file, value: _support/RustBuild.rs}]
// end-dotask
#[path = "_support/RustBuild.rs"]
mod rust_build;
use dotask_sdk::{serde_json, BuildContext, Context, Result, Value};
fn main() {
  dotask_sdk::run(task);
}
fn task(project: &BuildContext) -> Result<()> {
  let sdk = project.path("src/dotask-go");
  let cache = std::env::temp_dir().join("_go/dotask/sdk-tests");
  std::fs::create_dir_all(&cache)?;
  let mut command = project.command("go");
  command
    .current_dir(&sdk)
    .args(["test", "./..."])
    .env("GOTOOLCHAIN", "local")
    .env("GOWORK", "off")
    .env("GOENV", "off")
    .env("GOFLAGS", "")
    .env("CGO_ENABLED", "0")
    .env("GOPROXY", "off")
    .env("GOSUMDB", "off")
    .env("GOCACHE", &cache);
  project.execute(&mut command)?;
  let mut format = project.command("gofmt");
  format.arg("-l").arg(&sdk).arg(project.path("examples/go/.tasks"));
  let output = project.capture(&mut format)?;
  if !output.trim().is_empty() {
    dotask_sdk::bail!("Go source needs gofmt:\n{output}");
  }
  let tools = project
    .string("doinstall-tools")
    .ok()
    .filter(|v| !v.is_empty())
    .map(std::path::PathBuf::from)
    .or_else(|| std::env::var_os("DOINSTALL_TOOL_DIR").map(std::path::PathBuf::from))
    .context("Go installer acceptance requires --doinstall-tools or DOINSTALL_TOOL_DIR with prebuilt native tools")?;
  let target = rust_build::target_directory(project)?;
  let report: Value = serde_json::from_slice(&std::fs::read(target.join("rust-build/dotask-cli-executables.json"))?)?;
  // Retain the native gate's inspected build flags and time, then verify hashes.
  project.execute(
    project
      .command("cargo")
      .args([
        "test",
        "--release",
        "--locked",
        "--package",
        "dotask-cli",
        "--test",
        "go_tasks",
        "--",
        "--ignored",
      ])
      .env("DOINSTALL_TOOL_DIR", project.path(tools))
      .env_remove("RUSTFLAGS")
      .env(
        "CARGO_ENCODED_RUSTFLAGS",
        report["rustflags"].as_str().context("Missing inspected flags")?,
      )
      .env(
        "RUST_BUILD_TIMESTAMP",
        report["unix_seconds"]
          .as_u64()
          .context("Missing inspected timestamp")?
          .to_string(),
      ),
  )?;
  rust_build::inspected_binary(project)?;
  rust_build::inspected_executable(project, "dotask-doinstall-create-installer")?;
  Ok(())
}
