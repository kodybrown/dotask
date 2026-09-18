using DoTask;

/// <summary>Check the example's required configuration.</summary>
/// <requires setting="message" />
public static class Target
{
  public static void Main()
  {
    var project = BuildContext.Current;
    Console.WriteLine($"Configuration is available in {project.RootDirectory}");
  }
}
