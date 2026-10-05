// dotask: 1
// description: "Print a configurable greeting and host information."
// options:
//   - {"name": "name", "alias": "n", "type": "string", "default": "World", "description": "Who to greet."}
//   - {"name": "configuration", "alias": "c", "choices": ["Debug", "Release"], "default": "Debug", "description": "Build configuration."}
// requires:
//   - {"kind": "setting", "value": "message"}
// examples: ["dotask hello -n \"Ada Lovelace\" configuration=release"]
// end-dotask
using DoTask;


public static class Target
{
  public static void Main()
  {
    var project = BuildContext.Current;
    var config = project.Config;
    Console.WriteLine($"{config.Get<string>("message")}, {project.Parameters.Get<string>("name")}!");
    Console.WriteLine($"Host: {project.OS}; configuration: {project.Parameters.Get<string>("configuration")}");
  }
}
