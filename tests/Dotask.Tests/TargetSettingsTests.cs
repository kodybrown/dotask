namespace DoTask.Tests;

public sealed class TargetSettingsTests
{
  [Fact]
  public async Task RustCSharpAndYamlCallsResolveTheirOwnSettingsFromOneSnapshot()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", """
      settings:
        marker: shared
        nested: {retained: true, number: 1}
      targets:
        start:
          settings: {marker: parent, parent-only: private}
        managed:
          defaults: {label: yaml-default}
          settings: {marker: managed, nested: {number: 2}}
        group:
          settings: {marker: group}
        native:
          settings: {marker: native}
      """);
    project.Write(".tasks/start.rs", """
      use dotask_sdk::{json, BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        assert_eq!(p.setting("marker")?, "parent");
        std::fs::write(p.path(".dotasks.yaml"), "settings: {marker: changed}\n")?;
        p.exec_target("managed", json!({"label":"explicit"}))?;
        assert_eq!(p.setting("marker")?, "parent");
        Ok(())
      }
      """);
    project.Target("managed", """
      var p = BuildContext.Current;
      if (p.Config.Contains("parent-only")) {
        throw new Exception("Caller settings leaked.");
      }
      if (!p.Config.Get<bool>("nested.retained") || p.Config.Get<int>("nested.number") != 2) {
        throw new Exception("Shared mappings did not merge.");
      }
      p.Files.WriteText("managed.txt", p.Config.Get<string>("marker") + ":" + p.Parameters.Get<string>("label"));
      await p.ExecTargetAsync("group");
      """, "// dotask: 1\n// options: [{name: label, default: header}]\n// requires: [{kind: setting, value: marker}]\n// end-dotask\n", async: true);
    project.Write(".tasks/group.task", """
      steps:
        - file: {operation: write, path: group.txt, content: {setting: marker}}
        - run: native
        - run: tail
      """);
    project.Write(".tasks/native.rs", """
      use dotask_sdk::{BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        assert_eq!(p.setting("marker")?, "native");
        assert!(p.setting("parent-only").is_err());
        std::fs::write(p.path("native.txt"), p.setting("marker")?.as_str().unwrap())?;
        Ok(())
      }
      """);
    project.Target("tail", "BuildContext.Current.Files.WriteText(\"tail.txt\", BuildContext.Current.Config.Get<string>(\"marker\"));");

    var result = await project.RunAsync("start");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Equal("managed:explicit", File.ReadAllText(Path.Combine(project.Root, "managed.txt")));
    Assert.Equal("group", File.ReadAllText(Path.Combine(project.Root, "group.txt")));
    Assert.Equal("native", File.ReadAllText(Path.Combine(project.Root, "native.txt")));
    Assert.Equal("shared", File.ReadAllText(Path.Combine(project.Root, "tail.txt")));
  }

  [Fact]
  public async Task SharedTasksUseTheirFullIdentityAndQualifiedSettingsWhenCalledByShortcut()
  {
    using var project = new TestProject();
    project.Write(".dotasks.yaml", """
      settings: {marker: shared}
      targets:
        check:
          settings: {marker: wrong-shortcut}
        _/tools/check:
          settings: {marker: base, names: [one, two]}
        _/tools/check.cs:
          settings: {marker: qualified, output: result.txt}
      """);
    project.Target("_/tools/check", """
      var p = BuildContext.Current;
      p.Files.WriteText(p.Config.GetPath("output"), p.Config.Get<string>("marker") + ":" + string.Join(",", p.Config.Get<string[]>("names")));
      """, "// dotask: 1\n// requires: [{kind: setting, value: output}]\n// end-dotask\n");
    var result = await project.RunAsync("CHECK");
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Equal("qualified:one,two", File.ReadAllText(Path.Combine(project.Root, "result.txt")));
  }
}
