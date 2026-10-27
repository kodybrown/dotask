// dotask: 1
// description: "Refresh major.minor.yyMM.ddHH in Utah time, preserving preview unless explicitly incremented or clearing it for minor/major. Never build or commit."
// options:
//   - {"name":"project","type":"path","description":"Project file; otherwise settings.project."}
//   - {"name":"increment","type":"bool","default":"false","description":"Explicit date/hour refresh (also the default)."}
//   - {"name":"increment-preview","type":"bool","default":"false","description":"Increment preview.N, starting at preview.1 for a stable version."}
//   - {"name":"increment-minor","type":"bool","default":"false"}
//   - {"name":"increment-major","type":"bool","default":"false"}
//   - {"name":"dry-run","type":"bool","default":"false"}
// examples: ["dotask dotnet/update-version --dry-run", "dotask dotnet/update-version --increment-preview", "dotask dotnet/update-version --increment-minor"]
// requires: [{kind: file, value: dotnet/_support/CalendarVersion.cs}]
// end-dotask
#:include _support/CalendarVersion.cs
using System.Text;
using System.Text.RegularExpressions;
using System.Xml;
using System.Xml.Linq;
using DoTask;
using DoTask.Shared.DotNet;

public static class Target
{
  public static void Main()
  {
    var context = BuildContext.Current;
    var minor = context.Parameters.Get<bool>("increment-minor");
    var major = context.Parameters.Get<bool>("increment-major");
    var preview = context.Parameters.Get<bool>("increment-preview");
    var refresh = context.Parameters.Get<bool>("increment");
    if ((minor ? 1 : 0) + (major ? 1 : 0) + (preview ? 1 : 0) + (refresh ? 1 : 0) > 1)
      throw new TaskException("Choose only one version increment.");
    var path = context.Parameters.Contains("project")
        ? context.Parameters.GetPath("project") : context.Config.GetPath("project");
    var original = File.ReadAllBytes(path);
    // Edit only property value spans: XML serialization would rewrite comments,
    // whitespace, encoding/BOM and EOLs. XML line information excludes commented tags.
    using var stream = new MemoryStream(original);
    using var reader = new StreamReader(stream, new UTF8Encoding(false, true), true);
    var text = reader.ReadToEnd();
    var encoding = reader.CurrentEncoding;
    var root = XDocument.Parse(text, LoadOptions.SetLineInfo).Root
        ?? throw new TaskException("Missing project XML root.");
    if (root.Name != "Project"
        || root.Descendants().Any(element => element.Name.LocalName is "Import" or "VersionPrefix" or "VersionSuffix"))
      throw new TaskException("Expected an explicit Version; inherited or ambiguous versions are not supported.");

    var edits = new List<(int Index, int Length, string Value)>();
    var version = Property("Version", required: true)!;
    var current = version.Value.Trim();
    var next = CalendarVersion.Prepare(current, minor, major, preview, DateTimeOffset.UtcNow);
    var numeric = next.Split('-')[0];
    Replace(version, next);

    // AssemblyVersion is a binding contract, independent of release numbering.
    // Preserve every explicit assembly definition. Absent properties retain SDK
    // defaults; projects wanting stable binding must declare AssemblyVersion.
    var file = Property("FileVersion");
    if (file is not null) {
      if (!Regex.IsMatch(file.Value.Trim(), @"\A[0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?\z"))
        throw new TaskException("Expected an explicit numeric FileVersion.");
      Replace(file, numeric);
    }
    var informational = Property("InformationalVersion");
    if (informational is not null) {
      var value = informational.Value.Trim();
      // Keep Version-based MSBuild expressions live, including custom Git metadata.
      // The SDK may append SourceRevisionId later; never resolve Git here.
      if (!value.StartsWith("$(Version)", StringComparison.Ordinal)) {
        var match = Regex.Match(value,
          @"\A[0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?(?:-preview\.(?:0|[1-9][0-9]*))?(?<tail>.*)\z");
        if (!match.Success || match.Groups["tail"].Value.StartsWith("-preview", StringComparison.Ordinal))
          throw new TaskException("Expected InformationalVersion to start with a numeric version or $(Version).");
        Replace(informational, next + match.Groups["tail"].Value);
      }
    }

    foreach (var edit in edits)
      Console.WriteLine($"{path}: {text.Substring(edit.Index, edit.Length)} -> {edit.Value}");
    if (context.Parameters.Get<bool>("dry-run"))
      return;
    var updated = text;
    foreach (var edit in edits.OrderByDescending(edit => edit.Index))
      updated = updated[..edit.Index] + edit.Value + updated[(edit.Index + edit.Length)..];
    if (updated == text)
      return;
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
    // NuGet lockfiles do not store the root application's version.

    XElement? Property( string name, bool required = false )
    {
      var properties = root.Descendants().Where(element => element.Name.LocalName == name).ToArray();
      if (properties.Length == 0 && !required)
        return null;
      if (properties.Length != 1 || properties[0].Name != name
          || properties[0].Parent?.Name != "PropertyGroup"
          || properties[0].Parent?.Parent != root
          || properties[0].HasElements
          || properties[0].AncestorsAndSelf().Any(element => element.Attribute("Condition") is not null))
        throw new TaskException($"Expected one unconditional explicit {name}; inherited or ambiguous versions are not supported.");
      return properties[0];
    }

    void Replace( XElement element, string replacement )
    {
      var location = (IXmlLineInfo)element;
      var offset = 0;
      for (var line = 1; line < location.LineNumber; line++) {
        // XML normalizes CRLF and bare CR as a single newline.
        while (offset < text.Length && text[offset] is not ('\r' or '\n'))
          offset++;
        if (text[offset++] == '\r' && offset < text.Length && text[offset] == '\n')
          offset++;
      }
      // XElement's line position points at the tag name, after '<'.
      offset += location.LinePosition - 2;
      var match = Regex.Match(text[offset..],
        @"\A<" + element.Name.LocalName + @">\s*(?<value>[^<]*?)\s*</" + element.Name.LocalName + ">");
      if (!match.Success || match.Groups["value"].Value != element.Value.Trim())
        throw new TaskException($"Cannot safely locate the literal {element.Name} value; no update written.");
      var value = match.Groups["value"];
      edits.Add((offset + value.Index, value.Length, replacement));
    }
  }
}
