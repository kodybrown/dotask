using DoTask.Shared.DotNet;

namespace DoTask.Tests;

public sealed class CalendarVersionTests
{
  [Theory]
  [InlineData("2026-10-27T20:00:00Z", "1.3.2610.2714")]
  [InlineData("2026-01-15T21:00:00Z", "1.3.2601.1514")]
  [InlineData("2026-01-01T00:30:00Z", "1.3.2512.3117")]
  [InlineData("2028-03-01T06:30:00Z", "1.3.2802.2923")]
  [InlineData("2026-03-08T08:59:59Z", "1.3.2603.0801")]
  [InlineData("2026-03-08T09:00:00Z", "1.3.2603.0803")]
  [InlineData("2026-11-01T07:30:00Z", "1.3.2611.0101")]
  [InlineData("2026-11-01T08:30:00Z", "1.3.2611.0101")]
  public void UsesUtahCalendarIncludingDstAndDateBoundaries( string instant, string expected )
  {
    Assert.Equal(expected, CalendarVersion.Create(1, 3, false, false, DateTimeOffset.Parse(instant)));
  }

  [Theory]
  [InlineData(false, false, "1.3.2610.2714")]
  [InlineData(true, false, "1.4.2610.2714")]
  [InlineData(false, true, "2.0.2610.2714")]
  public void ExplicitIncrementsKeepCalendarSuffix( bool minor, bool major, string expected )
  {
    Assert.Equal(expected, CalendarVersion.Create(1, 3, minor, major,
      DateTimeOffset.Parse("2026-10-27T20:00:00Z")));
  }

  [Fact]
  public void RefusesComponentOverflowAndConflictingIncrements()
  {
    var now = DateTimeOffset.Parse("2026-10-27T20:00:00Z");
    Assert.Throws<ArgumentOutOfRangeException>(() => CalendarVersion.Create(65534, 0, false, true, now));
    Assert.Throws<ArgumentOutOfRangeException>(() => CalendarVersion.Create(1, 65534, true, false, now));
    Assert.Throws<ArgumentException>(() => CalendarVersion.Create(1, 3, true, true, now));
  }
}
