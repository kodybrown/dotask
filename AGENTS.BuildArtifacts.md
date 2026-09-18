# Shared Agent Module: Build Artifacts

<!-- Adapted from PowerCode build-artifacts v2026-08-24 for dotask. -->

- The user-level `~/Directory.Build.props` and `~/Directory.Build.targets` own
  .NET output locations. Do not replace, bypass, or override that policy.
- Keep generated `bin`, `obj`, and `publish` output outside source trees under
  `C:\tmp\_dotnet` on Windows and `/tmp/_dotnet` on Linux and macOS.
- Repository-level `Directory.Build.props` files must import the user-level
  props before defining other settings and must not redirect output into source.
- Build launchers must use the evaluated MSBuild `PublishDir`. Application
  installation uses the `dotnet/install` task and DoTask installation library;
  follow `docs/INSTALLATION.md` for versioned locations and command activation.
- Bootstrap runners are temporary physical copies of the publish output, outside
  the checkout. Do not redirect the repository's build outputs to stage a runner.
- Verify the evaluated `BaseOutputPath`, `BaseIntermediateOutputPath`, and
  `PublishDir` after build-tooling changes. Never add a source-local override
  to work around sandboxing, locks, or tooling.
