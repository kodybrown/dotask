// dotask: 1
// description: "Run another target, then write a project-relative output file."
// requires:
//   - {"kind": "setting", "value": "output"}
// end-dotask
using DoTask;


public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var config = project.Config;
    await project.ExecTargetAsync("check");
    await project.ExecTargetAsync("hello", new { Name = "Nested target", Configuration = "Release" });
    project.Files.WriteText(config.GetPath("output"), "Written by dotask.\n");
    Console.WriteLine(config.GetPath("output"));
  }
}
