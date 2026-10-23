namespace DoTask.Tests;

public sealed class ExecutionLanguageTests
{
  [Theory]
  [InlineData("--lang", "rust")]
  [InlineData("--lang=RuSt", null)]
  public async Task SelectsRootImplementationAndKeepsCrossLanguageNestedCallsIndependent( string selector, string? value )
  {
    using var project = Project();
    project.Target("tools/check", "Console.WriteLine(\"csharp-child\");", Metadata);
    project.Write(".tasks/tools/check.rs", Metadata + """
      use dotask_sdk::{json, BuildContext, Result};
      fn main() { dotask_sdk::run(task); }
      fn task(p: &BuildContext) -> Result<()> {
        println!("rust-root:{}", p.string("message")?);
        p.exec_target("tools/check.cs", json!({}))
      }
      """);
    string[] language = value is null ? [selector] : [selector, value];
    var result = await project.RunAsync(["tools/check", .. language, "message=--lang"]);
    Assert.True(result.ExitCode == 0, result.StandardError);
    Assert.Contains("rust-root:--lang", result.StandardOutput);
    Assert.Contains("csharp-child", result.StandardOutput);
    var before = await project.RunAsync([.. language, "CHECK", "message=before"]);
    Assert.True(before.ExitCode == 0, before.StandardError);
    Assert.Contains("rust-root:before", before.StandardOutput);
    project.Write(".dotasks.yaml", "invalid: [");
    var completions = NativeCompletion.Complete("dotask tools/check --lang rust --", project.Root);
    Assert.Contains(completions, c => c.Value == "--message");
    Assert.DoesNotContain(completions, c => c.Value == "--lang");
    Assert.Contains(NativeCompletion.Complete("dotask --lang rust tools/c", project.Root), c => c.Value == "tools/check");
    Assert.Contains(NativeCompletion.Complete("dotask tools/check --lang=r", project.Root), c => c.Value == "--lang=rust");
    Assert.Contains(NativeCompletion.Complete("dotask tools/check --lang c", project.Root), c => c.Value == "csharp");
    Assert.Contains(NativeCompletion.Complete("dotask tools/check.cs --", project.Root), c => c.Value == "--lang");
  }

  [Fact]
  public async Task HelpFiltersMetadataWithoutExecutingAndDirectoryNamesNeverBecomeExecutionGroups()
  {
    using var project = Project();
    project.Target("tools/check", "DoesNotCompile();", "// dotask: 1\n// description: CSharp variant.\n// end-dotask");
    project.Write(".tasks/tools/check.rs", "// dotask: 1\n// description: Rust variant.\n// end-dotask\nnot valid Rust");
    var target = await project.RunAsync("help", "tools/check", "--lang", "rust");
    Assert.Equal(0, target.ExitCode);
    Assert.Contains("Rust variant.", target.StandardOutput);
    Assert.DoesNotContain("CSharp variant.", target.StandardOutput);
    var summary = await project.RunAsync("--lang", "csharp");
    Assert.Equal(0, summary.ExitCode);
    Assert.Contains("CSharp variant.", summary.StandardOutput);
    Assert.DoesNotContain("Rust variant.", summary.StandardOutput);
    var directory = await project.RunAsync("tools", "--lang", "rust");
    Assert.Equal(1, directory.ExitCode);
    Assert.Contains("Unknown target 'tools'", directory.StandardError);
    Assert.DoesNotContain("Compilation failed", directory.StandardError);
  }

  [Theory]
  [InlineData("check.cs", "--lang=rust", "conflicts")]
  [InlineData("check.rs", "--lang=csharp", "conflicts")]
  [InlineData("only", "--lang=rust", "Unknown target")]
  [InlineData("check", "--lang=python", "csharp, rust or go")]
  [InlineData("check", "--lang=", "csharp, rust or go")]
  [InlineData("check", "--lang", "csharp, rust or go")]
  public async Task InvalidMissingAndConflictingSelectionsFailBeforeTaskSideEffects( string name, string selector, string diagnostic )
  {
    using var project = Project();
    project.Target("tools/check", "File.WriteAllText(\"executed\", \"bad\");", Metadata);
    project.Target("only", "File.WriteAllText(\"executed\", \"bad\");", Metadata);
    project.Write(".tasks/tools/check.rs", Metadata + "fn main() { panic!(\"must not run\"); }");
    var result = await project.RunAsync(name, selector);
    Assert.Equal(1, result.ExitCode);
    Assert.Contains(diagnostic, result.StandardError);
    Assert.False(File.Exists(Path.Combine(project.Root, "executed")));
    Assert.Equal(1, (await project.RunAsync("check", "--lang", "rust", "--lang=csharp")).ExitCode);
  }

  [Fact]
  public async Task AmbiguityRemainsAnErrorAndLangIsReservedAsATaskOption()
  {
    using var project = Project();
    project.Target("one/check", "File.WriteAllText(\"executed\", \"bad\");");
    project.Target("two/check", "File.WriteAllText(\"executed\", \"bad\");");
    Assert.Contains("Ambiguous", (await project.RunAsync("check", "--lang", "csharp")).StandardError);
    project.Target("invalid", "File.WriteAllText(\"executed\", \"bad\");", "// dotask: 1\n// options: [{name: lang}]\n// end-dotask");
    Assert.Contains("reserved", (await project.RunAsync("invalid")).StandardError);
    project.Write(".tasks/group.task", "steps: []");
    Assert.Equal(1, (await project.RunAsync("group", "--lang", "rust")).ExitCode);
    Assert.False(File.Exists(Path.Combine(project.Root, "executed")));
  }

  [Fact]
  public async Task GoSelectionCanNestRustCSharpAndGoWithIndependentLanguageResolution()
  {
    using var project = Project();
    project.Target("start", "throw new Exception(\"wrong language\");");
    project.Write(".tasks/start.go", "package main\nimport dotask \"dotask.local/sdk\"\nfunc main(){dotask.Run(func(c *dotask.BuildContext)error{return c.ExecTarget(\"middle.rs\",nil)})}\n");
    project.Write(".tasks/middle.rs", "fn main(){dotask_sdk::run(|c|c.exec_target(\"middle.cs\",dotask_sdk::json!({})));}\n");
    project.Target("middle", "await BuildContext.Current.ExecTargetAsync(\"leaf.go\");", async: true);
    project.Write(".tasks/leaf.go", "package main\nimport(\"fmt\";dotask \"dotask.local/sdk\")\nfunc main(){dotask.Run(func(c *dotask.BuildContext)error{fmt.Println(\"go-rust-csharp-go\");return nil})}\n");
    var result = await project.RunAsync("start", "--lang=Go");
    Assert.True(result.ExitCode == 0, result.StandardOutput + result.StandardError);
    Assert.Contains("go-rust-csharp-go", result.StandardOutput);
  }
  private const string Metadata = "// dotask: 1\n// options: [{name: message, default: default}]\n// end-dotask\n";
  private static TestProject Project() => new();
}
