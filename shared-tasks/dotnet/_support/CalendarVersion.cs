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
}
