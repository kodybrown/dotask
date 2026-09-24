using DoTask.Cli;

// Captured help/metadata must not depend on the Windows console's legacy code
// page. Pipelines and installer tooling consume the same Unicode project names.
if (Console.IsOutputRedirected || Console.IsErrorRedirected) {
  Console.OutputEncoding = new System.Text.UTF8Encoding(false);
}

using var cancellation = new CancellationTokenSource();
Console.CancelKeyPress += ( _, args ) =>
{
  args.Cancel = true;
  cancellation.Cancel();
};
return await CliApplication.RunAsync(args, Environment.CurrentDirectory, Console.Out, Console.Error, cancellation.Token);
