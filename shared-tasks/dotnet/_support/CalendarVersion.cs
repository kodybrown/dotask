using System.Globalization;

namespace DoTask.Shared.DotNet;

/// <summary>Creates the calendar version from one UTC instant in Utah's timezone.</summary>
internal static class CalendarVersion
{
  internal static string Create( int major, int minor, bool incrementMinor, bool incrementMajor, DateTimeOffset utcNow )
  {
    if (incrementMinor && incrementMajor)
      throw new ArgumentException("Choose only one version increment.");
    // Utah observes Mountain time with DST; never use the build machine's local
    // timezone or a fixed UTC offset. The fallback supports Windows without IANA mapping.
    TimeZoneInfo zone;
    try { zone = TimeZoneInfo.FindSystemTimeZoneById("America/Denver"); } catch (TimeZoneNotFoundException) { zone = TimeZoneInfo.FindSystemTimeZoneById("Mountain Standard Time"); }
    if (major is < 0 or > 65534 || minor is < 0 or > 65534)
      throw new ArgumentOutOfRangeException(nameof(major), "Version components must not exceed 65534.");
    if (incrementMajor) {
      if (major == 65534)
        throw new ArgumentOutOfRangeException(nameof(major), "Major increment exceeds 65534.");
      major++;
      minor = 0;
    } else if (incrementMinor) {
      if (minor == 65534)
        throw new ArgumentOutOfRangeException(nameof(minor), "Minor increment exceeds 65534.");
      minor++;
    }
    var utah = TimeZoneInfo.ConvertTime(utcNow, zone);
    return string.Create(CultureInfo.InvariantCulture, $"{major}.{minor}.{utah:yyMM}.{utah:ddHH}");
  }

  /// <summary>Refreshes a numeric/preview release without conflating binding or file versions.</summary>
  internal static string Prepare( string current, bool incrementMinor, bool incrementMajor,
    bool incrementPreview, DateTimeOffset utcNow )
  {
    if ((incrementMinor ? 1 : 0) + (incrementMajor ? 1 : 0) + (incrementPreview ? 1 : 0) > 1)
      throw new ArgumentException("Choose only one version increment.");
    var match = System.Text.RegularExpressions.Regex.Match(current,
      @"\A(?<numeric>[0-9]+\.[0-9]+\.[0-9]+(?:\.[0-9]+)?)(?:-preview\.(?<preview>0|[1-9][0-9]*))?\z");
    if (!match.Success)
      throw new ArgumentException("Expected a three- or four-component numeric Version, optionally followed by -preview.N.");
    var parts = match.Groups["numeric"].Value.Split('.');
    var components = new int[parts.Length];
    for (var index = 0; index < parts.Length; index++) {
      if (!int.TryParse(parts[index], NumberStyles.None, CultureInfo.InvariantCulture, out components[index])
          || components[index] > 65534)
        throw new ArgumentOutOfRangeException(nameof(current), "Version components must not exceed 65534.");
    }
    var suffix = match.Groups["preview"];
    var counter = 0;
    if (suffix.Success && !int.TryParse(suffix.Value, NumberStyles.None, CultureInfo.InvariantCulture, out counter))
      throw new ArgumentOutOfRangeException(nameof(current), "Preview counter exceeds Int32.MaxValue.");
    var numeric = Create(components[0], components[1], incrementMinor, incrementMajor, utcNow);
    if (incrementMinor || incrementMajor)
      return numeric;
    if (incrementPreview) {
      if (counter == int.MaxValue)
        throw new ArgumentOutOfRangeException(nameof(current), "Preview increment exceeds Int32.MaxValue.");
      return numeric + "-preview." + (counter + 1).ToString(CultureInfo.InvariantCulture);
    }
    return suffix.Success ? numeric + "-preview." + suffix.Value : numeric;
  }
}
