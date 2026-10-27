// dotask: 1
// description: "Increment patch by default, or explicitly choose minor/major. Never build or commit."
// options:
//   - {"name":"project","type":"path","description":"Project file; otherwise settings.project."}
//   - {"name":"increment","type":"bool","default":"false","description":"Explicit patch increment (also the default)."}
//   - {"name":"increment-minor","type":"bool","default":"false"}
//   - {"name":"increment-major","type":"bool","default":"false"}
//   - {"name":"dry-run","type":"bool","default":"false"}
// examples: ["dotask dotnet/update-version --dry-run", "dotask dotnet/update-version --increment-minor"]
// end-dotask
using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using DoTask;

public static class Target
{
  public static void Main()
  {
    var context = BuildContext.Current;
    var minor = context.Parameters.Get<bool>("increment-minor");
    var major = context.Parameters.Get<bool>("increment-major");
    var patch = context.Parameters.Get<bool>("increment");
    if ((minor ? 1 : 0) + (major ? 1 : 0) + (patch ? 1 : 0) > 1)
      throw new TaskException("Choose only one version increment.");
    var path = context.Parameters.Contains("project")
        ? context.Parameters.GetPath("project") : context.Config.GetPath("project");
    var original = File.ReadAllBytes(path);
    // Preserve the original encoding/BOM and replace only the numeric span:
    // XML reserialization would otherwise rewrite comments, whitespace and EOLs.
    using var stream = new MemoryStream(original);
    using var reader = new StreamReader(stream, new UTF8Encoding(false, true), true);
    var text = reader.ReadToEnd();
    var encoding = reader.CurrentEncoding;
    var root = XDocument.Parse(text).Root ?? throw new TaskException("Missing project XML root.");
    var versions = root.Descendants().Where(element => element.Name.LocalName == "Version").ToArray();
    if (root.Name.LocalName != "Project" || versions.Length != 1
        || versions[0].Parent?.Name.LocalName != "PropertyGroup"
        || versions[0].Parent?.Parent != root
        || versions[0].AncestorsAndSelf().Any(element => element.Attribute("Condition") is not null)
        || root.Descendants().Any(element => element.Name.LocalName is "Import" or "VersionPrefix" or "VersionSuffix"))
      throw new TaskException("Expected one unconditional explicit Version; inherited or ambiguous versions are not supported.");
    var matches = Regex.Matches(text, @"<Version>\s*(?<version>[0-9]+\.[0-9]+\.[0-9]+)\s*</Version>");
    if (matches.Count != 1 || matches[0].Groups["version"].Value != versions[0].Value.Trim())
      throw new TaskException("Expected one explicit MAJOR.MINOR.PATCH Version.");
    var value = matches[0].Groups["version"];
    var parts = value.Value.Split('.').Select(part => int.Parse(part, CultureInfo.InvariantCulture)).ToArray();
    // SDK-generated assembly/file versions have 16-bit components; 65535 is reserved.
    if (parts.Any(part => part > 65534))
      throw new TaskException("Version components must not exceed 65534.");
    var index = major ? 0 : minor ? 1 : 2;
    if (parts[index] == 65534)
      throw new TaskException("Version increment exceeds the .NET limit (65534).");
    parts[index]++;
    for (var suffix = index + 1; suffix < parts.Length; suffix++)
      parts[suffix] = 0;
    var next = string.Join(".", parts);
    Console.WriteLine($"{path}: {value.Value} -> {next}");
    if (context.Parameters.Get<bool>("dry-run"))
      return;
    var updated = text[..value.Index] + next + text[(value.Index + value.Length)..];
    var preamble = encoding.GetPreamble();
    var result = encoding.GetBytes(updated);
    if (preamble.Length > 0 && original.AsSpan().StartsWith(preamble))
      result = preamble.Concat(result).ToArray();
    if (!File.ReadAllBytes(path).AsSpan().SequenceEqual(original))
      throw new TaskException("Project changed during version preparation; no update written.");
    var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
    try {
      File.WriteAllBytes(temporary, result);
      File.Move(temporary, path, overwrite: true);
    } finally {
      if (File.Exists(temporary))
        File.Delete(temporary);
    }
    // NuGet lockfiles do not store this application's version; no dependency
    // or project-reference entry needs a version edit for a manifest bump.
  }
}
