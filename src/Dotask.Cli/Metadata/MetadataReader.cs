using System.Text.RegularExpressions;
using System.Xml;
using System.Xml.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

namespace DoTask.Cli.Metadata;

public static partial class MetadataReader
{
  public static TargetDefinition Read( string file, string? taskDirectory = null )
  {
    var name = Path.GetFileNameWithoutExtension(file);
    string? shortName = null;
    try {
      var parts = name.Split(' ');
      if (parts.Length > 2 || parts.Any(part => !Identifier().IsMatch(part))) {
        throw new TaskException($"Invalid target filename '{Path.GetFileName(file)}'. Expected '<target>.cs' or '<group> <target>.cs' with one space.");
      }
      if (parts.Length == 2) {
        shortName = parts[1];
        name = parts[0] + "-" + shortName;
      }
      if (taskDirectory is not null) {
        var relative = Path.GetRelativePath(taskDirectory, file).Replace(Path.DirectorySeparatorChar, '/');
        var parents = relative.Split('/')[..^1];
        if (parents.Any(parent => !Identifier().IsMatch(parent))) {
          throw new TaskException($"Invalid target directory in '{relative}'. Use letters, digits, underscores, or hyphens, starting with a letter.");
        }
        if (parents.Length > 0) {
          shortName ??= name;
          name = string.Join('/', parents.Append(name));
        }
      }
      if (ReservedCommands.Contains(name, StringComparer.OrdinalIgnoreCase)) {
        throw new TaskException($"Invalid or reserved target name '{name}'.");
      }
      var root = CSharpSyntaxTree.ParseText(File.ReadAllText(file),
        new CSharpParseOptions(LanguageVersion.Preview, DocumentationMode.Parse)).GetRoot();
      var main = root.DescendantNodes().OfType<MethodDeclarationSyntax>()
        .FirstOrDefault(m => m.Identifier.ValueText == "Main" && m.Modifiers.Any(SyntaxKind.StaticKeyword));
      var type = main?.Ancestors().OfType<TypeDeclarationSyntax>().FirstOrDefault();
      var comments = Documentation(type) ?? Documentation(main);
      if (comments is null) {
        return new(name, file, "(no description)", [], [], [], null, [], ShortName: shortName);
      }
      var content = string.Join('\n', comments.Split('\n').Select(line =>
      {
        var text = line.TrimStart();
        return text.StartsWith("///") ? text[3..] : text;
      }));
      if (content.TrimStart().StartsWith("/**")) {
        content = content.Trim()[3..^2];
        content = string.Join('\n', content.Split('\n').Select(line => line.TrimStart().TrimStart('*')));
      }
      using var reader = XmlReader.Create(new StringReader("<target>" + content + "</target>"),
        new XmlReaderSettings { DtdProcessing = DtdProcessing.Prohibit, XmlResolver = null });
      var xml = XElement.Load(reader);
      var options = xml.Elements("option").Select(ReadOption).ToArray();
      var tokens = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
      foreach (var option in options) {
        foreach (var token in new[] { option.Name, option.Alias }.OfType<string>()) {
          if (!tokens.Add(token) || ReservedOptions.Contains(token, StringComparer.OrdinalIgnoreCase)) {
            throw new TaskException($"Duplicate or reserved option/alias '{token}'.");
          }
        }
      }
      var requirements = xml.Elements("requires").Select(element =>
      {
        var attributes = element.Attributes().ToArray();
        if (attributes.Length != 1 || attributes[0].Name.Namespace != XNamespace.None
          || attributes[0].Name.LocalName is not ("tool" or "setting" or "os" or "task" or "file")
          || string.IsNullOrWhiteSpace(attributes[0].Value)) {
          throw new TaskException("<requires> must specify one tool, setting, os, task, or file attribute.");
        }
        return new Requirement(attributes[0].Name.LocalName, attributes[0].Value);
      }).ToArray();
      return new(name, file, Text(xml.Element("summary")) ?? "(no description)", options, requirements,
        xml.Elements("capability").Select(e => (string?)e.Attribute("name") ?? Text(e) ?? "").ToArray(),
        Text(xml.Element("remarks")), xml.Elements("example").Select(e => Text(e) ?? "").ToArray(), ShortName: shortName);
    } catch (Exception ex) when (ex is XmlException or TaskException or IOException or UnauthorizedAccessException) {
      return new(name, file, "", [], [], [], null, [], $"Metadata error: {ex.Message}", shortName);
    }
  }

  public static readonly string[] ReservedCommands = ["help", "completion", "__complete", "__exec"];
  public static readonly string[] ReservedOptions = ["help", "h", "use-dir", "version"];

  private static string? Documentation( SyntaxNode? node ) => node?.GetLeadingTrivia()
    .FirstOrDefault(t => t.IsKind(SyntaxKind.SingleLineDocumentationCommentTrivia)
      || t.IsKind(SyntaxKind.MultiLineDocumentationCommentTrivia)).ToFullString() is { Length: > 0 } text ? text : null;

  private static OptionDefinition ReadOption( XElement element )
  {
    var name = (string?)element.Attribute("name") ?? "";
    var alias = (string?)element.Attribute("alias");
    var type = ((string?)element.Attribute("type") ?? "string").ToLowerInvariant();
    var required = (string?)element.Attribute("required") ?? "false";
    var completion = (string?)element.Attribute("completion");
    if (!Identifier().IsMatch(name)) {
      throw new TaskException($"Invalid option name '{name}'.");
    }
    if (alias is not null && (alias.Length != 1 || !char.IsAsciiLetter(alias[0]))) {
      throw new TaskException($"Option '{name}' must have a single-letter alias.");
    }
    if (type is not ("string" or "bool" or "int" or "number" or "path")) {
      throw new TaskException($"Option '{name}' has unsupported type '{type}'.");
    }
    if (!bool.TryParse(required, out var isRequired)) {
      throw new TaskException($"Option '{name}' has an invalid required attribute.");
    }
    if (completion is not null and not ("file" or "directory")) {
      throw new TaskException($"Option '{name}' has unsupported completion '{completion}'.");
    }
    return new(name, alias, type, Text(element) ?? "", (string?)element.Attribute("default"), isRequired,
      ((string?)element.Attribute("choices") ?? "").Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries), completion);
  }

  private static string? Text( XElement? element ) => element is null ? null : Whitespace().Replace(element.Value.Trim(), " ");
  [GeneratedRegex("^[a-zA-Z][a-zA-Z0-9_-]*$")]
  private static partial Regex Identifier();
  [GeneratedRegex(@"\s+")]
  private static partial Regex Whitespace();
}
