using DoTask;

/// <summary>Print a configurable greeting and host information.</summary>
/// <option name="name" alias="n" type="string" default="World">Who to greet.</option>
/// <option name="configuration" alias="c" choices="Debug,Release" default="Debug">Build configuration.</option>
/// <requires setting="message" />
/// <example>dotask hello -n "Ada Lovelace" configuration=release</example>
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
