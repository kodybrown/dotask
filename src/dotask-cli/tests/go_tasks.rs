//! Consumer acceptance uses physical prebuilt CLI/helper/SDK snapshots. Only Go
//! and separately supplied native doinstall tools are visible to task processes.
use serde_json::{json, Value};
use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  sync::OnceLock,
};
fn repository() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn success(output: Output) -> Output {
  assert!(
    output.status.success(),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  output
}
fn compiler() -> &'static PathBuf {
  static GO: OnceLock<PathBuf> = OnceLock::new();
  GO.get_or_init(|| {
    let output = success(
      Command::new("go")
        .args(["env", "GOROOT"])
        .env("GOTOOLCHAIN", "local")
        .env("GOENV", "off")
        .env("GOWORK", "off")
        .output()
        .expect("Go acceptance requires an installed Go 1.22+ compiler"),
    );
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim()).join("bin")
  })
}
struct Fixture {
  _temporary: tempfile::TempDir,
  root: PathBuf,
  dist: PathBuf,
  traps: PathBuf,
}
impl Fixture {
  fn new() -> Self {
    let temporary = tempfile::Builder::new().prefix("dotask-go-acceptance-").tempdir().unwrap();
    let root = temporary.path().join("consumer spaces 日本語");
    let dist = temporary.path().join("prebuilt dotask 日本語");
    fs::create_dir_all(root.join(".tasks")).unwrap();
    fs::create_dir(&dist).unwrap();
    for (source, name) in [
      (env!("CARGO_BIN_EXE_dotask"), "dotask"),
      (
        env!("CARGO_BIN_EXE_dotask-doinstall-create-installer"),
        "dotask-doinstall-create-installer",
      ),
    ] {
      fs::copy(source, dist.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))).unwrap();
    }
    for name in [
      "go.mod",
      "context.go",
      "process.go",
      "tasks.go",
      "installers.go",
    ] {
      fs::create_dir_all(dist.join("sdk/go")).unwrap();
      fs::copy(repository().join("src/dotask-go").join(name), dist.join("sdk/go").join(name)).unwrap();
    }
    let traps = temporary.path().join("forbidden tools");
    fs::create_dir(&traps).unwrap();
    Self {
      _temporary: temporary,
      root,
      dist,
      traps,
    }
  }
  fn write(&self, name: &str, text: &str) {
    let path = self.root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
  }
  fn command(&self, args: &[&str]) -> Command {
    let mut command = Command::new(self.dist.join(format!("dotask{}", std::env::consts::EXE_SUFFIX)));
    let mut directories = vec![self.traps.clone(), compiler().clone()];
    // Native macOS packaging uses the OS codesign utility. Keep language-tool
    // traps first so adding system utilities cannot hide a Rust/.NET dependency.
    if cfg!(target_os = "macos") {
      directories.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    }
    let path = std::env::join_paths(directories).unwrap();
    command
      .current_dir(&self.root)
      .args(args)
      .env("PATH", path)
      .env("DOTASK_CACHE_HOME", self._temporary.path().join("cache"))
      .env("DOTASK_PRIVATE_TASKS", self._temporary.path().join("private"))
      .env("DOTASK_ONLINE_TASKS", self._temporary.path().join("offline"))
      .env("DOTASK_SOURCE_SDK_STAGE", "missing-sdk")
      .env("DOTASK_SOURCE_ROOT", "missing-root")
      .env("DOTASK_SOURCE_RUST_SDK", "missing-rust")
      .env("DOTNET_HOST_PATH", "missing-dotnet")
      // Hostile consumer settings must neither select another compiler/target nor
      // read consumer modules, download anything, or write build output in source.
      .env("GOTOOLCHAIN", "go9.99.0+auto")
      .env("GOWORK", self.root.join("hostile.work"))
      .env("GOFLAGS", "-invalid-consumer-flag")
      .env("GOCACHEPROG", "cargo")
      .env("GOOS", "unknown")
      .env("GOARCH", "unknown")
      .env("GOCACHE", self.root.join("must-not-write-cache"))
      .env("FORBIDDEN_TOOL_LOG", self.root.join("forbidden.log"))
      .env_remove("DOTASK_PROCESS_GROUP_OWNER")
      .env_remove("DOINSTALL_PROCESS_GROUP_OWNER");
    command
  }
  fn run(&self, args: &[&str]) -> Output {
    self.command(args).output().unwrap()
  }
  fn traps(&self) {
    // Compile this fixture once with the installed Go compiler. Any attempted
    // Rust/Cargo/.NET launch during acceptance is observable, even on a host that
    // has those developer tools installed outside the restricted process PATH.
    let source = self._temporary.path().join("trap.go");
    fs::write(&source,"package main\nimport(\"os\";\"path/filepath\")\nfunc main(){ f,_:=os.OpenFile(os.Getenv(\"FORBIDDEN_TOOL_LOG\"),os.O_CREATE|os.O_APPEND|os.O_WRONLY,0600); if f!=nil {f.WriteString(filepath.Base(os.Args[0])+\"\\n\");f.Close()};os.Exit(97)}\n").unwrap();
    let trap = self.traps.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
    success(
      Command::new(compiler().join(format!("go{}", std::env::consts::EXE_SUFFIX)))
        .args(["build", "-o"])
        .arg(&trap)
        .arg(source)
        .env("GOTOOLCHAIN", "local")
        .env("GOENV", "off")
        .env("GOWORK", "off")
        .env("CGO_ENABLED", "0")
        .output()
        .unwrap(),
    );
    for name in ["rustc", "rustup", "dotnet"] {
      fs::copy(&trap, self.traps.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))).unwrap();
    }
  }
}
fn tree(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
  fn visit(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for entry in fs::read_dir(path).unwrap() {
      let entry = entry.unwrap();
      if entry.file_type().unwrap().is_dir() {
        visit(root, &entry.path(), out)
      } else {
        out.insert(entry.path().strip_prefix(root).unwrap().into(), fs::read(entry.path()).unwrap());
      }
    }
  }
  let mut result = BTreeMap::new();
  visit(path, path, &mut result);
  result
}
#[test]
fn go_metadata_identity_completion_and_shared_management_need_no_compiler() {
  let f = Fixture::new();
  f.write(
    ".dotasks.yaml",
    "version: 1\ntargets:\n  echo:\n    defaults: {name: base}\n  echo.go:\n    defaults: {name: Go-specific}\n",
  );
  f.write(".tasks/echo.go","// dotask: 1\n// description: Go metadata 日本語\n// options: [{name: name}]\n// end-dotask\nthis deliberately does not compile\n");
  f.write(
    ".tasks/echo.rs",
    "// dotask: 1\n// description: Rust implementation\n// end-dotask\ninvalid\n",
  );
  f.write(
    ".tasks/echo.cs",
    "// dotask: 1\n// description: CSharp implementation\n// end-dotask\ninvalid\n",
  );
  let run = |args: &[&str]| f.command(args).env("PATH", "").output().unwrap();
  let help = success(run(&["help", "echo", "--lang", "go"]));
  let text = String::from_utf8(help.stdout).unwrap();
  assert!(
    text.contains("Go metadata 日本語") && text.contains("Go-specific") && text.contains("requires Go"),
    "{text}"
  );
  assert!(!run(&["echo"]).status.success());
  assert!(!run(&["echo.rs", "--lang", "go"]).status.success());
  let complete = success(run(&["__complete", "--line", "dotask echo."]));
  assert!(String::from_utf8_lossy(&complete.stdout).contains("echo.go"));
  let private = f._temporary.path().join("private/tools");
  fs::create_dir_all(&private).unwrap();
  fs::write(private.join("hello.go"),"// dotask: 1\n// requires: [{kind: file, value: tools/_support/helper.go}]\n// end-dotask\npackage main\nfunc main(){}\n").unwrap();
  fs::create_dir(private.join("_support")).unwrap();
  fs::write(private.join("_support/helper.go"), "package helper\n").unwrap();
  success(run(&["--add", "private-tasks/tools/hello", "--lang", "go"]));
  assert!(f.root.join(".tasks/private-tasks/tools/hello.go").is_file());
  fs::write(
    private.join("hello.go"),
    "// dotask: 1\n// description: Updated Go task\n// end-dotask\npackage main\nfunc main(){}\n",
  )
  .unwrap();
  success(run(&["--sync", "private-tasks/tools/hello", "--lang", "go"]));
  assert!(fs::read_to_string(f.root.join(".tasks/private-tasks/tools/hello.go"))
    .unwrap()
    .contains("Updated Go"));
  success(run(&["--remove", "private-tasks/tools/hello", "--lang", "go"]));
  assert!(!f.root.join(".tasks/private-tasks/tools/hello.go").exists());
}
const PROBE: &str = r#"// dotask: 1
// options: [{name: mode, default: normal}]
// requires: [{kind: file, value: _support/message/message.go}]
// end-dotask
package main
import("encoding/json";"errors";"fmt";"os";"time"; dotask "dotask.local/sdk";"dotask.local/task/_support/message")
func main(){if len(os.Args)>1 && os.Args[1]=="probe" {data,_:=json.Marshal(os.Args[2:]);fmt.Println(string(data));return}; dotask.Run(task)}
func task(c *dotask.BuildContext) error {
 mode,_:=c.String("mode")
 if mode=="fail" {return fmt.Errorf("wrapped: %w",&dotask.ProcessFailure{Code:37,Cause:errors.New("expected")})}
 if mode=="sleep" {os.WriteFile(c.Root()+"/ready",[]byte("ready"),0600);for {time.Sleep(time.Second)}}
 if mode=="cycle" {return c.ExecTarget("probe.go",map[string]any{"mode":"cycle"})}
 self,_:=os.Executable(); text,err:=c.Capture(c.Command(self,"probe","","space value","日本語",`quote" trailing\`));if err!=nil{return err};fmt.Print(text)
 if err=c.ExecTarget("child.go",map[string]any{"count":int64(2147483647)});err!=nil{return err}
 absent,err:=c.ExecIfExists("absent",nil);if err!=nil || absent.Exists{return errors.New("absence contract failed")}
 ambiguous,err:=c.ExecIfExists("ambiguous",nil);if err!=nil || !ambiguous.Exists || ambiguous.Error==nil{return errors.New("ambiguity contract failed")}
 fmt.Println(message.Text);return nil
}
"#;
#[test]
fn go_execution_is_offline_isolated_preserves_arguments_and_invalidates_sources_and_sdk() {
  let f = Fixture::new();
  f.traps();
  f.write(
    ".dotasks.yaml",
    "version: 1\nsettings: {marker: shared}\ntargets:\n  probe:\n    settings: {marker: caller}\n",
  );
  f.write("go.mod", "module consumer.example/untouched\ngo 9.99.0\n");
  f.write("go.sum", "consumer owned\n");
  f.write("go.work", "go 9.99.0\n");
  f.write(".tasks/probe.go", PROBE);
  f.write(
    ".tasks/_support/message/message.go",
    "package message\nconst Text=\"support-one\"\n",
  );
  f.write(".tasks/child.go",r#"// dotask: 1
// options: [{name: count, type: int}]
// end-dotask
package main
import("fmt";dotask "dotask.local/sdk")
func main(){dotask.Run(func(c *dotask.BuildContext)error {n,err:=c.Int("count");if err!=nil{return err};s,err:=dotask.SettingAs[string](c,"marker");if err!=nil{return err};fmt.Println(n,s);return nil})}
"#);
  f.write(".tasks/ambiguous.go", "invalid");
  f.write(".tasks/ambiguous.task", "steps: []");
  let before = tree(&f.root);
  let first = success(f.run(&["probe", "--lang", "go"]));
  let text = String::from_utf8(first.stdout).unwrap();
  assert!(text.contains(r#"["","space value","日本語","quote\" trailing\\"]"#), "{text}");
  assert!(text.contains("2147483647 shared") && text.contains("support-one"), "{text}");
  success(f.run(&["probe.go"]));
  assert_eq!(before, tree(&f.root));
  f.write(
    ".tasks/_support/message/message.go",
    "package message\nconst Text=\"support-two\"\n",
  );
  assert!(String::from_utf8_lossy(&success(f.run(&["probe.go"])).stdout).contains("support-two"));
  // Changing an SDK implementation must invalidate the executable cache too.
  let sdk = f.dist.join("sdk/go/tasks.go");
  let original = fs::read_to_string(&sdk).unwrap();
  fs::write(&sdk,original.replace("func (c *BuildContext) ExecTarget(target string, parameters any) error {", "func (c *BuildContext) ExecTarget(target string, parameters any) error { return fmt.Errorf(\"SDK invalidated\");")).unwrap();
  let changed = f.run(&["probe.go"]);
  assert!(!changed.status.success());
  assert!(
    String::from_utf8_lossy(&changed.stderr).contains("SDK invalidated"),
    "{changed:?}"
  );
  fs::write(sdk, original).unwrap();
  assert_eq!(f.run(&["probe.go", "--mode", "fail"]).status.code(), Some(37));
  let cycle = f.run(&["probe.go", "--mode", "cycle"]);
  assert!(!cycle.status.success());
  assert!(String::from_utf8_lossy(&cycle.stderr).contains("Target cycle"));
  assert!(!f.root.join("forbidden.log").exists());
  assert!(!f.dist.join("sdk/rust").exists());
  assert!(!f.root.join("must-not-write-cache").exists());
}
#[test]
#[ignore = "requires explicit prebuilt doinstall tools; the required test-go gate runs this acceptance"]
fn go_tasks_create_return_and_record_real_installers_using_only_prebuilt_tools() {
  let f = Fixture::new();
  f.traps();
  let tools = PathBuf::from(
    std::env::var_os("DOINSTALL_TOOL_DIR")
      .expect("Go installer acceptance requires DOINSTALL_TOOL_DIR with prebuilt native tools"),
  );
  for name in ["doinstall", "doinstall-builder", "doinstall-launcher"] {
    fs::copy(
      tools.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
      f.dist.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
  }
  for name in ["create-installer.task", "assemble-self-extracting.task"] {
    f.write(
      &format!(".tasks/_/doinstall/{name}"),
      &fs::read_to_string(repository().join("shared-tasks/doinstall").join(name)).unwrap(),
    );
  }
  f.write(".tasks/stage.go",r#"// dotask: 1
// options: [{name: payload, type: path}]
// end-dotask
package main
import("os";"path/filepath";dotask "dotask.local/sdk")
func main(){if len(os.Args)>1 && os.Args[1]=="--version" {println("Go consumer 1.0");return};dotask.Run(func(c *dotask.BuildContext)error{path,err:=c.String("payload");if err!=nil{return err};if err=os.MkdirAll(path,0700);err!=nil{return err};self,err:=os.Executable();if err!=nil{return err};data,err:=os.ReadFile(self);if err!=nil{return err};if err=os.WriteFile(filepath.Join(path,"app"+filepath.Ext(self)),data,0755);err!=nil{return err};return os.WriteFile(filepath.Join(path,"note.txt"),[]byte("Go payload 日本語"),0600)})}
"#);
  f.write(".tasks/package.go",r#"package main
import("errors";"fmt";dotask "dotask.local/sdk")
func main(){dotask.Run(func(c *dotask.BuildContext)error {
 a,err:=c.CreateInstaller("_/doinstall/create-installer.task",nil);if err!=nil{return err}
 found,err:=c.FindInstaller();if err!=nil{return err};if found==nil || found.FilePath!=a.FilePath{return errors.New("record mismatch")}
 if err=c.RecordInstaller(a);err!=nil{return err}
 if err=c.RunInstaller(a,[]string{"--extract-to",a.FilePath+"-extracted","--","--validate"});err!=nil{return err}
 fmt.Println("artifact:",a.FilePath);return c.SetInstallerResult(a)
})}
"#);
  let build = |version: &str| json!({"schema":1,"application":{"id":"go-acceptance","name":"Go acceptance","version":version},"runtime":{"interactive":false,"commands":[{"name":"go-acceptance","executable":"app${exe-extension}"}]},"build-steps":[{"run":"stage.go","with":{"payload":"${payload}"}},{"run":"_/doinstall/assemble-self-extracting.task","with":{"builder":"${builder}","installer":"${installer}","launcher-template":"${launcher-template}","config":"${config}","output":"${package}","result-file":"${result-file}"}}]});
  f.write("installer.yaml", &build("1.0.0").to_string());
  f.write(".dotasks.yaml",&json!({"version":1,"settings":{"app":{"git-hash":"none","dirty-suffix":false},"installer":{"config":"installer.yaml","output":"packages"},"doinstall":{"builder":f.dist.join(format!("doinstall-builder{}",std::env::consts::EXE_SUFFIX)),"installer":f.dist.join(format!("doinstall{}",std::env::consts::EXE_SUFFIX)),"launcher-template":f.dist.join(format!("doinstall-launcher{}",std::env::consts::EXE_SUFFIX))}},"targets":{"_/doinstall/create-installer":{"defaults":{"build-stamp":"26296-1200"}}}}).to_string());
  let result = success(f.run(&["package.go"]));
  assert!(String::from_utf8_lossy(&result.stdout).contains("artifact:"));
  let record = f.root.join(format!(
    "artifacts/.dotask/installers/{}-{}.json",
    std::env::consts::OS,
    if cfg!(target_arch = "x86_64") {
      "x64"
    } else {
      "arm64"
    }
  ));
  let saved = fs::read(&record).unwrap();
  let value: Value = serde_json::from_slice(&saved).unwrap();
  assert!(Path::new(value["Installer"]["FilePath"].as_str().unwrap()).is_file());
  let mut failure = build("2.0.0");
  failure["build-steps"]
    .as_array_mut()
    .unwrap()
    .push(json!({"run":"reject.task"}));
  f.write(".tasks/reject.task", "steps: [{exec: {executable: forbidden-final-step}}]\n");
  f.write("installer.yaml", &failure.to_string());
  assert!(!f.run(&["package.go"]).status.success());
  assert_eq!(fs::read(record).unwrap(), saved);
  assert!(!f.root.join("forbidden.log").exists());
  assert!(!f.root.join("go.mod").exists());
  assert!(!f.root.join("go.sum").exists());
  assert!(!f.dist.join("sdk/dotnet").exists());
}

#[test]
fn missing_compiler_and_unsupported_versions_fail_without_downloads() {
  let f = Fixture::new();
  f.write(".dotasks.yaml", "version: 1\n");
  f.write(".tasks/task.go", "package main\nfunc main(){}\n");
  let missing = f.command(&["task.go"]).env("PATH", "").output().unwrap();
  assert!(!missing.status.success());
  assert!(String::from_utf8_lossy(&missing.stderr).contains("installed Go 1.22+"));
  f.traps();
  let fake = f.traps.join("fake.go");
  fs::write(&fake,"package main\nimport(\"fmt\";\"os\")\nfunc main(){if os.Getenv(\"GOTOOLCHAIN\")!=\"local\" {os.Exit(91)};fmt.Print(`{\"GOVERSION\":\"go1.21.9\"}`)}\n").unwrap();
  success(
    Command::new(compiler().join(format!("go{}", std::env::consts::EXE_SUFFIX)))
      .args(["build", "-o"])
      .arg(f.traps.join(format!("go{}", std::env::consts::EXE_SUFFIX)))
      .arg(fake)
      .env("GOTOOLCHAIN", "local")
      .env("GOWORK", "off")
      .env("GOENV", "off")
      .env("CGO_ENABLED", "0")
      .output()
      .unwrap(),
  );
  let unsupported = f.run(&["task.go"]);
  assert!(!unsupported.status.success());
  assert!(
    String::from_utf8_lossy(&unsupported.stderr).contains("Unsupported Go compiler 'go1.21.9'"),
    "{unsupported:?}"
  );
}

#[test]
fn concurrent_go_builds_do_not_publish_partial_binaries_and_source_changes_recompile() {
  let f = Fixture::new();
  f.write(".dotasks.yaml", "version: 1\n");
  f.write(".tasks/once.GO", "package main\nfunc main(){println(\"first-source\")}\n");
  let children: Vec<_> = (0..3)
    .map(|_| {
      f.command(&["once.go"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
    })
    .collect();
  for child in children {
    let result = success(child.wait_with_output().unwrap());
    assert!(String::from_utf8_lossy(&result.stderr).contains("first-source"));
  }
  f.write(".tasks/once.GO", "package main\nfunc main(){println(\"changed-source\")}\n");
  let result = success(f.run(&["once.go"]));
  assert!(String::from_utf8_lossy(&result.stderr).contains("changed-source"));
}

#[test]
fn shared_go_support_packages_survive_installation_and_updates() {
  let f = Fixture::new();
  f.write(".dotasks.yaml", "version: 1\n");
  let private = f._temporary.path().join("private/tools");
  fs::create_dir_all(private.join("_support/message")).unwrap();
  fs::write(private.join("say.go"),"// dotask: 1\n// requires: [{kind: file, value: tools/_support/message/message.go}]\n// end-dotask\npackage main\nimport(\"fmt\";dotask \"dotask.local/sdk\";\"dotask.local/task/_support/message\")\nfunc main(){dotask.Run(func(c *dotask.BuildContext)error{fmt.Println(message.Text);return nil})}\n").unwrap();
  fs::write(
    private.join("_support/message/message.go"),
    "package message\nconst Text=\"shared-Go-one\"\n",
  )
  .unwrap();
  success(f.run(&["--add", "private-tasks/tools/say", "--lang", "go"]));
  let result = success(f.run(&["private-tasks/tools/say.go"]));
  assert!(String::from_utf8_lossy(&result.stdout).contains("shared-Go-one"));
  fs::write(
    private.join("_support/message/message.go"),
    "package message\nconst Text=\"shared-Go-two\"\n",
  )
  .unwrap();
  success(f.run(&["--sync", "private-tasks/tools/say", "--lang", "go"]));
  let result = success(f.run(&["private-tasks/tools/say.go"]));
  assert!(String::from_utf8_lossy(&result.stdout).contains("shared-Go-two"));
  // A locally edited installed helper remains consumer-owned.
  f.write(
    ".tasks/private-tasks/tools/_support/message/message.go",
    "package message\nconst Text=\"local\"\n",
  );
  success(f.run(&["--sync", "private-tasks/tools/say", "--lang", "go"]));
  let installed = f.root.join(".tasks/private-tasks/tools/_support/message/message.go");
  let local = fs::read(&installed).unwrap();
  assert!(String::from_utf8_lossy(&local).contains("local"));
  fs::write(
    private.join("_support/message/message.go"),
    "package message\nconst Text=\"shared-Go-three\"\n",
  )
  .unwrap();
  assert!(!f.run(&["--sync", "private-tasks/tools/say", "--lang", "go"]).status.success());
  assert_eq!(fs::read(installed).unwrap(), local);
}

#[test]
#[ignore = "requires explicit prebuilt doinstall tools; the required test-go gate runs this acceptance"]
fn shared_go_installer_tasks_package_a_configured_consumer_without_rust_or_dotnet() {
  let f = Fixture::new();
  f.traps();
  let tools = PathBuf::from(std::env::var_os("DOINSTALL_TOOL_DIR").expect("prebuilt native tools"));
  for name in ["doinstall", "doinstall-builder", "doinstall-launcher"] {
    fs::copy(
      tools.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
      f.dist.join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
  }
  // Exercise real catalog installation and rewritten group-relative support paths.
  for args in [
    ["--save", "go/create-installer.go"],
    ["--add", "go/create-installer.go"],
    ["--save", "go/test-installer.go"],
    ["--add", "go/test-installer.go"],
  ] {
    success(
      f.command(&args)
        .env("DOTASK_ONLINE_TASKS", repository().join("shared-tasks"))
        .output()
        .unwrap(),
    );
  }
  let executable = format!("widget{}", std::env::consts::EXE_SUFFIX);
  let settings = json!({"version":1,"settings":{
    "go":{"package":"./cmd/widget","executable":executable,"version-file":"cmd/widget/version.go","version-symbol":"Version","version-output":"Widget version {version}","version-arguments":["--version","argument spaces 日本語","","quote\"","C:\\a b\\"],"installer-files":["docs/notice.txt"],"installer-forbidden-files":[".gostatic"],"clean-environment-prefixes":["WIDGET_"]},
    "app":{"git-hash":"none","dirty-suffix":false},
    "installer":{"config":"installer.yaml","output":"deliveries spaces 日本語"},
    "doinstall":{"builder":f.dist.join(format!("doinstall-builder{}",std::env::consts::EXE_SUFFIX)),"installer":f.dist.join(format!("doinstall{}",std::env::consts::EXE_SUFFIX)),"launcher-template":f.dist.join(format!("doinstall-launcher{}",std::env::consts::EXE_SUFFIX))}
  },"targets":{"_/doinstall/create-installer":{"defaults":{"build-stamp":"26300-1200"}}}});
  f.write(".dotasks.yaml", &settings.to_string());
  let module = "module example.invalid/widget\n\ngo 1.22.0\n";
  let version="package main\n// A grouped raw literal and trailing comment must survive unchanged.\nconst (Version = `v2.3.4` /* release */)\n";
  f.write("go.mod", module);
  f.write("cmd/widget/version.go", version);
  f.write("cmd/widget/main.go",r#"package main
import("fmt";"os";"strings";"reflect")
func main(){if !reflect.DeepEqual(os.Args[1:],[]string{"--version","argument spaces 日本語","","quote\"","C:\\a b\\"}){os.Exit(37)};if os.Getenv("WIDGET_SPOIL")!="" {fmt.Println("site settings leaked");return};fmt.Println("Widget version "+strings.TrimPrefix(Version,"v"))}
"#);
  f.write("docs/notice.txt", "payload 日本語\n");
  f.write(".gostatic", "site-owned and excluded");
  let recipe = json!({"schema":1,"application":{"id":"go-widget","name":"Go widget"},"runtime":{"interactive":false,"commands":[{"name":"widget","executable":executable}]},"build-steps":[
    {"run":"_/go/stage-installer.go","with":{"stage-dir":"${staging}/app","app-version":"${version}"},"gather":[{"from":"${staging}/app","files":[executable,"docs/notice.txt"]}]},
    {"run":"_/doinstall/assemble-self-extracting.task","with":{"builder":"${builder}","installer":"${installer}","launcher-template":"${launcher-template}","config":"${config}","output":"${package}","result-file":"${result-file}"}}
  ]});
  f.write("installer.yaml", &recipe.to_string());
  f.write(".tasks/package.go",r#"package main
import("errors";"fmt";dotask "dotask.local/sdk")
func main(){dotask.Run(func(c *dotask.BuildContext)error{
 a,err:=c.CreateInstaller("_/go/create-installer.go",nil);if err!=nil{return err}
 found,err:=c.FindInstaller();if err!=nil{return err};if found==nil || found.FilePath!=a.FilePath{return errors.New("record mismatch")}
 if err=c.RunInstaller(a,[]string{"--extract-to",a.FilePath+"-extracted","--","--validate"});err!=nil{return err}
 fmt.Println("shared-artifact:",a.FilePath);return c.SetInstallerResult(a)
})}
"#);
  let result = success(
    f.command(&["package.go"])
      .env("WIDGET_SPOIL", "must not reach version smoke")
      .output()
      .unwrap(),
  );
  assert!(String::from_utf8_lossy(&result.stdout).contains("shared-artifact:"));
  let record = f.root.join(format!(
    "artifacts/.dotask/installers/{}-{}.json",
    std::env::consts::OS,
    if cfg!(target_arch = "x86_64") {
      "x64"
    } else {
      "arm64"
    }
  ));
  let saved = fs::read(&record).unwrap();
  let artifact: Value = serde_json::from_slice(&saved).unwrap();
  let delivery = Path::new(artifact["Installer"]["FilePath"].as_str().unwrap());
  assert!(delivery.is_file());
  if cfg!(windows) {
    let checked = success(
      f.command(&["_/go/test-installer.go"])
        .env("WIDGET_SPOIL", "must be cleared")
        .output()
        .unwrap(),
    );
    assert!(String::from_utf8_lossy(&checked.stdout).contains("PASS: user/machine PATH unchanged"));
    assert!(String::from_utf8_lossy(&checked.stdout).contains(delivery.to_str().unwrap()));
    fs::remove_file(&record).unwrap();
    let absent = f.run(&["_/go/test-installer.go"]);
    assert!(!absent.status.success());
    assert!(String::from_utf8_lossy(&absent.stderr).contains("no successful installer is recorded"));
    let mut invalid_record = artifact.clone();
    invalid_record["Sha256"] = json!("0".repeat(64));
    let invalid = serde_json::to_vec(&invalid_record).unwrap();
    fs::write(&record, &invalid).unwrap();
    let changed = f.run(&["_/go/test-installer.go"]);
    assert!(!changed.status.success());
    assert!(String::from_utf8_lossy(&changed.stderr).contains("has changed since its successful build"));
    // An explicit artifact remains usable even when a different prior record
    // is unavailable or invalid; testing does not replace or repair that record.
    success(f.run(&[
      "_/go/test-installer.go",
      "--installer",
      delivery.to_str().unwrap(),
    ]));
    assert_eq!(fs::read(&record).unwrap(), invalid);
    fs::write(&record, &saved).unwrap();
    let clean_failure = |failed: Output, code: i32, diagnostic: &str| {
      assert_eq!(failed.status.code(), Some(code), "{}", String::from_utf8_lossy(&failed.stderr));
      let error = String::from_utf8_lossy(&failed.stderr);
      assert!(error.contains(diagnostic), "{error}");
      let retained = error
        .lines()
        .find_map(|line| line.strip_prefix("Failed acceptance fixture retained: "))
        .expect("failure must report retained logs");
      let retained = PathBuf::from(retained).canonicalize().unwrap();
      assert!(retained.is_dir());
      assert!(retained.parent() == Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
      assert!(retained
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("dotask-installer-test-"));
      assert!(retained.join("process-1.stdout").is_file());
      let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
      loop {
        match fs::remove_dir_all(&retained) {
          Ok(()) => break,
          Err(error) if matches!(error.raw_os_error(), Some(5 | 32 | 33)) && std::time::Instant::now() < deadline => {
            std::thread::sleep(std::time::Duration::from_millis(50))
          }
          Err(error) => panic!("Failed to clean verified acceptance fixture: {error}"),
        }
      }
    };
    clean_failure(
      f.run(&[
        "_/go/test-installer.go",
        "--installer",
        delivery.to_str().unwrap(),
        "--app-version",
        "9.9.9",
      ]),
      1,
      "unexpected installed version",
    );
    let fake_source = f._temporary.path().join("failed-installer.go");
    fs::write(&fake_source,r#"package main
import("os";"os/exec";"time")
func main(){if os.Getenv("ACCEPTANCE_HANG")!="1"{os.Exit(23)};if len(os.Args)>1 && os.Args[1]=="--child"{for{os.WriteFile(os.Getenv("ACCEPTANCE_MARKER"),[]byte(time.Now().String()),0600);time.Sleep(20*time.Millisecond)}};cmd:=exec.Command(os.Args[0],"--child");if err:=cmd.Start();err!=nil{panic(err)};time.Sleep(time.Hour)}
"#).unwrap();
    let fake = f._temporary.path().join("failed-installer.exe");
    success(
      Command::new(compiler().join("go.exe"))
        .args(["build", "-o"])
        .arg(&fake)
        .arg(fake_source)
        .env("GOTOOLCHAIN", "local")
        .env("GOENV", "off")
        .env("GOWORK", "off")
        .env("CGO_ENABLED", "0")
        .output()
        .unwrap(),
    );
    clean_failure(
      f.run(&[
        "_/go/test-installer.go",
        "--installer",
        fake.to_str().unwrap(),
      ]),
      23,
      "failed; logs under",
    );
    let marker = f.root.join("timeout-child.marker");
    let timed_out = f
      .command(&[
        "_/go/test-installer.go",
        "--installer",
        fake.to_str().unwrap(),
        "--timeout-seconds",
        "1",
      ])
      .env("ACCEPTANCE_HANG", "1")
      .env("ACCEPTANCE_MARKER", &marker)
      .output()
      .unwrap();
    clean_failure(timed_out, 1, "context deadline exceeded");
    let last = fs::read(&marker).expect("timeout fixture child must start");
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert_eq!(
      fs::read(&marker).unwrap(),
      last,
      "timed-out installer descendant survived task cleanup"
    );
  } else {
    let unsupported = f.run(&[
      "_/go/test-installer.go",
      "--installer",
      delivery.to_str().unwrap(),
    ]);
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("requires native Windows"));
  }
  let extracted = PathBuf::from(format!("{}-extracted", delivery.display()));
  let inventory = tree(&extracted);
  assert!(inventory
    .iter()
    .any(|(p, b)| p.ends_with("docs/notice.txt") && b == "payload 日本語\n".as_bytes()));
  assert!(!inventory.keys().any(|p| p.file_name().unwrap() == ".gostatic"));
  assert_eq!(fs::read_to_string(f.root.join("go.mod")).unwrap(), module);
  assert_eq!(fs::read_to_string(f.root.join("cmd/widget/version.go")).unwrap(), version);
  assert!(!f.root.join("go.sum").exists());
  let external = f._temporary.path().join("stage 日本語");
  let stage = external.to_str().unwrap();
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    stage,
    "--app-version",
    "wrong",
  ]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("version does not match"));
  assert!(!external.join("docs/notice.txt").exists());
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    stage,
    "--app-version",
    "2.3.4",
  ]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("must be fresh"));
  let local = f.root.join("source-stage");
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    local.to_str().unwrap(),
    "--app-version",
    "2.3.4",
  ]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("outside the project"));
  assert!(!local.exists());
  f.write(
    "cmd/widget/version.go",
    "package main\nvar Version = \"v2.3.4\" + \"computed\"\n",
  );
  let failed = f.run(&["_/go/create-installer.go"]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("must be a string literal"));
  f.write("cmd/widget/version.go", version);
  let mut traversal = settings.clone();
  traversal["settings"]["go"]["installer-files"] = json!(["../outside.txt"]);
  f.write(".dotasks.yaml", &traversal.to_string());
  let fresh = f._temporary.path().join("fresh-stage");
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    fresh.to_str().unwrap(),
    "--app-version",
    "2.3.4",
  ]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("must stay beneath"));
  assert!(!fresh.exists());
  let mut bad_arguments = settings.clone();
  bad_arguments["settings"]["go"]["version-arguments"] = json!([]);
  f.write(".dotasks.yaml", &bad_arguments.to_string());
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    fresh.to_str().unwrap(),
    "--app-version",
    "2.3.4",
  ]);
  assert_eq!(failed.status.code(), Some(37), "{}", String::from_utf8_lossy(&failed.stderr));
  let newer = f._temporary.path().join("newer-toolchain-stage");
  f.write(".dotasks.yaml", &settings.to_string());
  f.write("go.mod", "module example.invalid/widget\n\ngo 99.0.0\n");
  let failed = f.run(&[
    "_/go/stage-installer.go",
    "--stage-dir",
    newer.to_str().unwrap(),
    "--app-version",
    "2.3.4",
  ]);
  assert!(!failed.status.success());
  assert!(String::from_utf8_lossy(&failed.stderr).contains("GOTOOLCHAIN=local"));
  assert!(!String::from_utf8_lossy(&failed.stderr).contains("downloading go"));
  assert_eq!(fs::read(&record).unwrap(), saved);
  assert!(!f.root.join("forbidden.log").exists());
  assert!(!f.dist.join("sdk/rust").exists());
  assert!(!f.dist.join("sdk/dotnet").exists());
}

#[test]
fn shared_go_version_preparation_preserves_source_bytes_without_other_toolchains() {
  let f = Fixture::new();
  f.traps();
  for args in [
    ["--save", "go/update-version.go"],
    ["--add", "go/update-version.go"],
  ] {
    success(
      f.command(&args)
        .env("DOTASK_ONLINE_TASKS", repository().join("shared-tasks"))
        .output()
        .unwrap(),
    );
  }
  f.write(
    ".dotasks.yaml",
    &json!({"version":1,"settings":{"go":{"version-file":"version.go","version-symbol":"Version"}}}).to_string(),
  );
  let source="\u{feff}// 日本語 header\r\npackage main\r\nvar Version = \"v1.2.3-preview.4+custom.007\" // preserve\r\nvar Other = \"unrelated\"\r\n";
  f.write("version.go", source);
  let module = "module example.invalid/unchanged\n\ngo 99.0.0\n";
  f.write("go.mod", module);
  success(f.run(&["_/go/update-version.go", "--dry-run"]));
  assert_eq!(fs::read(f.root.join("version.go")).unwrap(), source.as_bytes());
  success(f.run(&["_/go/update-version.go", "--increment-preview"]));
  let preview = source.replace("v1.2.3-preview.4+custom.007", "v1.2.3-preview.5+custom.007");
  assert_eq!(fs::read(f.root.join("version.go")).unwrap(), preview.as_bytes());
  success(f.run(&["_/go/update-version.go", "--increment-minor"]));
  let minor = source.replace("v1.2.3-preview.4+custom.007", "v1.3.0+custom.007");
  assert_eq!(fs::read(f.root.join("version.go")).unwrap(), minor.as_bytes());
  let failed = f.run(&[
    "_/go/update-version.go",
    "--increment",
    "--increment-preview",
  ]);
  assert!(!failed.status.success());
  assert_eq!(fs::read(f.root.join("version.go")).unwrap(), minor.as_bytes());
  let raw = "package main\nconst Version = `v2.3.4` // untouched\n";
  f.write("version.go", raw);
  success(f.run(&["_/go/update-version.go"]));
  assert_eq!(
    fs::read_to_string(f.root.join("version.go")).unwrap(),
    raw.replace("v2.3.4", "v2.3.5")
  );
  let computed = "package main\nvar Version = \"1.2.\" + \"3\"\n";
  f.write("version.go", computed);
  assert!(!f.run(&["_/go/update-version.go"]).status.success());
  assert_eq!(fs::read_to_string(f.root.join("version.go")).unwrap(), computed);
  let legacy = "package main\nvar Version = \"v1.5.261023.956\"\n";
  f.write("version.go", legacy);
  assert!(!f.run(&["_/go/update-version.go"]).status.success());
  assert_eq!(fs::read_to_string(f.root.join("version.go")).unwrap(), legacy);
  let dated = "package main\nvar Version = \"v1.5.261023\"\n";
  f.write("version.go", dated);
  let checked = success(f.run(&[
    "_/go/update-version.go",
    "--patch-mode",
    "date",
    "--dry-run",
  ]));
  assert!(String::from_utf8_lossy(&checked.stdout).contains("v1.5.26"));
  assert_eq!(fs::read_to_string(f.root.join("version.go")).unwrap(), dated);
  assert_eq!(fs::read_to_string(f.root.join("go.mod")).unwrap(), module);
  assert!(!f.root.join("go.sum").exists());
  assert!(!f.root.join("forbidden.log").exists());
  assert_eq!(
    fs::read_dir(&f.root)
      .unwrap()
      .filter_map(Result::ok)
      .filter(|e| e.file_name().to_string_lossy().starts_with(".dotask-version-"))
      .count(),
    0
  );
}
