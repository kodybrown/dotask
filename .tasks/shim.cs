using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using DoTask;

/// <summary>Build the bundled Windows shims with LLVM, or verify their recorded hashes.</summary>
/// <option name="verify" type="bool" default="false">Check source and binary hashes without requiring LLVM.</option>
/// <example>dotask shim --verify</example>
public static class Target
{
  public static async Task Main()
  {
    var project = BuildContext.Current;
    var root = project.Path("src", "Dotask.Shim");
    var assets = Path.Combine(root, "assets");
    var sources = new[] { "shim.c", "kernel32.def" };
    var architectures = new[] { (Name: "x64", Triple: "x86_64", Machine: "i386:x86-64"),
      (Name: "arm64", Triple: "aarch64", Machine: "arm64") };
    if (!project.Parameters.Get<bool>("verify")) {
      var temporary = Directory.CreateTempSubdirectory("dotask-shim-").FullName;
      try {
        Directory.CreateDirectory(assets);
        foreach (var arch in architectures) {
          var library = Path.Combine(temporary, $"kernel32-{arch.Name}.lib");
          var obj = Path.Combine(temporary, $"shim-{arch.Name}.obj");
          await project.RunAsync("llvm-dlltool", ["-m", arch.Machine, "-d", Path.Combine(root, "kernel32.def"), "-l", library]);
          await project.RunAsync("clang", [$"--target={arch.Triple}-pc-windows-msvc", "-c", Path.Combine(root, "shim.c"),
            "-o", obj, "-Os", "-ffreestanding", "-fno-builtin", "-fno-stack-protector", "-Wall", "-Wextra", "-Werror"]);
          await project.RunAsync("lld-link", [obj, library, "/entry:mainCRTStartup", "/subsystem:console", "/nodefaultlib",
            "/dynamicbase", "/nxcompat", "/timestamp:0", "/Brepro", $"/out:{Path.Combine(assets, $"win-{arch.Name}.exe")}"]);
        }
      } finally { Directory.Delete(temporary, recursive: true); }
    }
    var names = sources.Concat(architectures.Select(a => $"assets/win-{a.Name}.exe"));
    var hashes = names.ToDictionary(name => name, name => Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(Path.Combine(root, name)))));
    hashes.Add("build-task.cs", Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(project.TargetFile))));
    var record = JsonSerializer.Serialize(hashes, new JsonSerializerOptions { WriteIndented = true }).ReplaceLineEndings("\n") + "\n";
    var manifest = Path.Combine(root, "assets", "hashes.json");
    if (project.Parameters.Get<bool>("verify")) {
      if (!File.Exists(manifest) || File.ReadAllText(manifest) != record)
        throw new TaskException("Shim sources or bundled binaries changed. Rebuild with dotask shim and review the binaries and hashes together.");
      Console.WriteLine("Bundled shim hashes match their recorded sources and binaries.");
    } else
      File.WriteAllText(manifest, record, new UTF8Encoding(false));
  }
}
