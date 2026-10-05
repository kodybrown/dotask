// dotask: 1
// description: "Check the example's required configuration."
// requires:
//   - {"kind": "setting", "value": "message"}
// end-dotask
using DoTask;


public static class Target
{
  public static void Main()
  {
    var project = BuildContext.Current;
    Console.WriteLine($"Configuration is available in {project.RootDirectory}");
  }
}
