//! Contracts migrated from the retired managed CLI's internal unit tests.
//! Exercise the maintained implementation directly, without a second parser.
use crate::{catalog, configuration, project::Directory, task_metadata};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn write(root: &Path, name: &str, text: &str) {
  let file = root.join(name);
  fs::create_dir_all(file.parent().unwrap()).unwrap();
  fs::write(file, text).unwrap();
}
fn fixture() -> (tempfile::TempDir, Directory) {
  let root = tempfile::tempdir().unwrap();
  fs::create_dir(root.path().join(".tasks")).unwrap();
  let directory = Directory::locate(root.path().into(), None).unwrap();
  (root, directory)
}
fn config(directory: &Directory, text: &str) -> anyhow::Result<Value> {
  write(&directory.root_directory, ".dotasks.yaml", text);
  configuration::load(directory)
}
const OPTIONS: &str = "// dotask: 1\n// options:\n//   - {name: configuration, alias: c, choices: [Debug, Release], default: Debug}\n//   - {name: name, alias: n}\n//   - {name: count, type: int}\n//   - {name: detailed, alias: v, type: bool}\n//   - {name: output, type: path}\n// end-dotask\n";

#[test]
fn optional_identity_and_typed_settings_are_separate() {
  let (_root, directory) = fixture();
  for text in [
    "version: 1",
    "name: null\ndescription: null",
    "name: ''\ndescription: '  '",
  ] {
    let value = config(&directory, text).unwrap();
    assert!(value["Name"].is_null());
    assert!(value["Description"].is_null());
  }
  let value = config(&directory, "NAME: '  Example project  '\nDescription: |\n  First line.\n  Second line.\nsettings:\n  name: Shared value\n  app: {path: 'src/My App/app.csproj', retries: 3, enabled: true, literal: 'true', names: [one, two]}").unwrap();
  assert_eq!(value["Name"], "Example project");
  assert_eq!(value["Description"], "First line.\nSecond line.");
  assert_eq!(value["Settings"]["name"], "Shared value");
  assert_eq!(configuration::find(&value["Settings"], "APP.Retries"), Some(&json!(3)));
  assert_eq!(value["Settings"]["app"]["enabled"], true);
  assert_eq!(value["Settings"]["app"]["literal"], "true");
  assert_eq!(value["Settings"]["app"]["names"], json!(["one", "two"]));
  assert!(configuration::defaults(&value, "anything").unwrap().is_empty());
}

#[test]
fn invalid_identity_and_configuration_fail_clearly() {
  let (_root, directory) = fixture();
  for text in [
    "name: 42",
    "name: [one, two]",
    "description: true",
    "description: {text: value}",
    "settings: {Name: one, NAME: two}",
    "version: 2",
    "settings: [one, two]",
    "unexpected: true",
    "targets: {run: {description: no}}",
    "settings: &a {next: *a}",
  ] {
    assert!(config(&directory, text).is_err(), "{text}");
  }
  config(&directory, "settings: {}").unwrap();
  write(&directory.task_directory, "config.yaml", "settings: {}");
  assert!(configuration::load(&directory).unwrap_err().to_string().contains("Both"));
}

#[test]
fn binding_preserves_precedence_aliases_types_and_literal_values() {
  let (_root, directory) = fixture();
  write(&directory.task_directory, "run.cs", OPTIONS);
  let target = task_metadata::metadata(&directory.task_directory.join("run.cs"), &directory.task_directory);
  let value = config(&directory, "targets: {RUN: {defaults: {Configuration: Release}}}").unwrap();
  let bind = |args: &[&str], help| {
    configuration::bind(
      &target,
      &value,
      &args.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
      &directory.root_directory,
      help,
    )
  };
  assert_eq!(bind(&[], false).unwrap()["configuration"], "Release");
  for args in [
    vec!["--configuration", "debug"],
    vec!["-C", "debug"],
    vec!["--CONFIGURATION=debug"],
    vec!["Configuration=debug"],
    vec!["C=debug"],
  ] {
    assert_eq!(bind(&args, false).unwrap()["configuration"], "Debug");
  }
  let bound = bind(
    &[
      "name=A=B=c",
      "--count",
      "-42",
      "-v",
      "output=some\\Path.txt",
    ],
    false,
  )
  .unwrap();
  assert_eq!(bound["name"], "A=B=c");
  assert_eq!(bound["count"], -42);
  assert_eq!(bound["detailed"], true);
  assert_eq!(
    bound["output"],
    directory.root_directory.join("some").join("Path.txt").to_str().unwrap()
  );
  assert_eq!(bind(&["name="], false).unwrap()["name"], "");
  for args in [
    vec!["configuration=invalid"],
    vec!["missing=value"],
    vec!["count=abc"],
    vec!["detailed=maybe"],
    vec!["--name"],
    vec!["-c", "Debug", "configuration=Release"],
  ] {
    assert!(bind(&args, false).is_err(), "{args:?}");
  }
  let value = config(&directory, "targets: {run: {defaults: {confguration: Release}}}").unwrap();
  assert!(configuration::bind(&target, &value, &[], &directory.root_directory, false).is_err());
  write(
    &directory.task_directory,
    "required.cs",
    "// dotask: 1\n// options: [{name: name, required: true}]\n// end-dotask",
  );
  let required = task_metadata::metadata(&directory.task_directory.join("required.cs"), &directory.task_directory);
  let empty = json!({"Settings": {}, "TargetDefaults": {}});
  assert!(configuration::bind(&required, &empty, &[], &directory.root_directory, false).is_err());
  assert!(configuration::bind(&required, &empty, &[], &directory.root_directory, true).is_ok());
}

#[test]
fn discovery_excludes_helpers_and_documents_but_accepts_both_languages_and_groups() {
  let (_root, directory) = fixture();
  for name in ["_/dotnet/build.cs", "run.rs", "group.task"] {
    write(
      &directory.task_directory,
      name,
      if name.ends_with("task") {
        "steps: []"
      } else {
        ""
      },
    );
  }
  for name in [
    "_support/helper.cs",
    "_/dotnet/_support/helper.cs",
    "_/dotnet/_/helper.cs",
    "tools/_/helper.cs",
    "_hidden/build.cs",
    "_.cs",
    "misc/bootstrap.txt",
    "misc/bootstrap.md",
    "misc/bootstrap.targets",
    "misc/bootstrap.yaml",
    "misc/bootstrap.py",
    "bin/run.cs",
    "obj/run.rs",
  ] {
    write(
      &directory.task_directory,
      name,
      "// dotask: 1\n// description: Excluded\n// end-dotask",
    );
  }
  let catalog = catalog::load(&directory.task_directory, directory.root_directory.as_path()).unwrap();
  assert_eq!(catalog.targets.len(), 3);
  assert_eq!(catalog.find("dotnet/build").unwrap().unwrap().name, "_/dotnet/build");
  assert_eq!(catalog.find("BUILD").unwrap().unwrap().name, "_/dotnet/build");
  assert_eq!(catalog::sources(&directory.task_directory, true, false).unwrap().len(), 1);
}

#[test]
fn discovery_preserves_invocation_and_exact_directory_override() {
  let (_root, directory) = fixture();
  let child = directory.root_directory.join("src/child");
  fs::create_dir_all(&child).unwrap();
  let found = Directory::locate(child.clone(), None).unwrap();
  assert_eq!(found.root_directory, directory.root_directory);
  assert_eq!(found.invocation_directory, child);
  fs::create_dir(directory.root_directory.join("src/.tasks")).unwrap();
  assert_eq!(
    Directory::locate(child.clone(), None).unwrap().root_directory,
    directory.root_directory.join("src")
  );
  assert!(Directory::locate(child.clone(), Some("missing")).is_err());
  fs::create_dir(child.join(".abc")).unwrap();
  assert_eq!(
    Directory::locate(child.clone(), Some(".abc")).unwrap().task_directory,
    child.join(".abc")
  );
  fs::remove_dir_all(&directory.task_directory).unwrap();
  config(&directory, "name: Root project").unwrap();
  let found = Directory::locate(directory.root_directory.clone(), None).unwrap();
  assert_eq!(found.root_directory, directory.root_directory);
  assert!(catalog::load(&found.task_directory, &found.root_directory)
    .unwrap()
    .targets
    .is_empty());
}

#[test]
fn filenames_shortcuts_collisions_and_exact_precedence_share_one_catalog() {
  for (filename, full, short) in [
    ("run", "run", None),
    ("format-check", "format-check", None),
    ("dotnet-run", "dotnet-run", None),
    ("dotnet run", "dotnet-run", Some("run")),
    ("DotNet format-check", "DotNet-format-check", Some("format-check")),
  ] {
    let (_root, directory) = fixture();
    write(&directory.task_directory, &format!("{filename}.cs"), "");
    let catalog = catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
    let target = catalog.find(&full.to_uppercase()).unwrap().unwrap();
    assert_eq!(target.name, full);
    assert_eq!(target.short_name.as_deref(), short);
    if let Some(short) = short {
      assert_eq!(catalog.find(&short.to_uppercase()).unwrap().unwrap().name, full);
    }
  }
  for filename in [
    "dotnet  run",
    "dotnet run check",
    " dotnet-run",
    "dotnet-run ",
    "dotnet\trun",
    "dotnet^run",
  ] {
    let (_root, directory) = fixture();
    assert!(catalog::read_name(
      &directory.task_directory.join(format!("{filename}.cs")),
      &directory.task_directory
    )
    .is_err());
  }
  let (_root, directory) = fixture();
  write(&directory.task_directory, "dotnet format.cs", "");
  write(&directory.task_directory, "rust format.cs", "");
  let load = || catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
  assert!(load().find("FORMAT").is_err());
  write(&directory.task_directory, "format.cs", "");
  assert_eq!(load().find("FORMAT").unwrap().unwrap().name, "format");
  for (first, second) in [
    ("dotnet run", "dotnet-run"),
    ("dotnet format-check", "dotnet-format check"),
  ] {
    let (_root, directory) = fixture();
    write(&directory.task_directory, &format!("{first}.cs"), "");
    write(&directory.task_directory, &format!("{second}.cs"), "");
    let catalog = catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
    assert!(catalog.targets.iter().all(|t| t.error.as_ref().unwrap().contains("Ambiguous")));
    let error = catalog.find(&catalog.targets[0].name).err().unwrap().to_string();
    assert!(error.contains("Ambiguous full target name"));
    assert!(error.contains(&format!("{first}.cs")));
    assert!(error.contains(&format!("{second}.cs")));
  }
}

#[test]
fn full_names_precede_shortcuts_and_reserved_commands_are_never_shortcuts() {
  let (_root, directory) = fixture();
  for name in [
    "dotnet run",
    "other dotnet-run",
    "dotnet help",
    "dotnet completion",
  ] {
    write(&directory.task_directory, &format!("{name}.cs"), "");
  }
  let catalog = catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
  assert_eq!(catalog.find("dotnet-run").unwrap().unwrap().name, "dotnet-run");
  for name in ["help", "completion"] {
    assert!(catalog.find(name).unwrap().is_none());
    let target = catalog.find(&format!("dotnet-{name}")).unwrap().unwrap();
    assert!(target.error.is_none());
    assert_eq!(catalog.names(target), [format!("dotnet-{name}")]);
  }
}

#[test]
fn case_only_collisions_are_reported_when_the_filesystem_can_represent_them() {
  let (_root, directory) = fixture();
  write(&directory.task_directory, "run.cs", "");
  write(&directory.task_directory, "RUN.cs", "");
  let catalog = catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
  if catalog.targets.len() == 2 {
    assert!(catalog.find("run").is_err());
    assert!(catalog.targets.iter().all(|t| t.error.as_ref().unwrap().contains("Ambiguous")));
  }
}

#[test]
fn invalid_metadata_is_isolated_and_good_targets_remain_callable() {
  let (_root, directory) = fixture();
  for yaml in [
    "options: [{name: configuration, alias: cc}]",
    "options: [{name: help}]",
    "options: [{name: a, alias: c}, {name: C}]",
    "options: [{name: x, type: date}]",
    "[invalid",
    "requires: [{kind: invalid, value: invalid}]",
    "requires: [{kind: task, value: ''}]",
    "requires: [{kind: task, value: x, extra: y}]",
  ] {
    write(
      &directory.task_directory,
      "broken.cs",
      &format!("// dotask: 1\n// {yaml}\n// end-dotask"),
    );
    write(&directory.task_directory, "good.cs", "");
    let catalog = catalog::load(&directory.task_directory, &directory.root_directory).unwrap();
    assert!(
      catalog.targets.iter().find(|t| t.name == "broken").unwrap().error.is_some(),
      "{yaml}"
    );
    assert_eq!(catalog.find("GOOD").unwrap().unwrap().name, "good");
  }
}
