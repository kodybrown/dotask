using DoTask.Cli;

using var cancellation = new CancellationTokenSource();
Console.CancelKeyPress += (_, args) =>
{
  args.Cancel = true;
  cancellation.Cancel();
};
return await CliApplication.RunAsync(args, Environment.CurrentDirectory, Console.Out, Console.Error, cancellation.Token);
