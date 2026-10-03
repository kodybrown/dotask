namespace DoTask.Tests;

// Run the same ownership, dependency, synchronization and recovery scenarios
// against the real native executable. Reference-side readers deliberately stay
// C# so these cases also verify the shared catalog/lockfile wire formats.
public sealed class RustSharedTaskTests : SharedTaskTests
{
  protected override bool NativeRunner => true;
}
