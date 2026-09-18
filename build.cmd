@echo off
setlocal DisableDelayedExpansion
pushd "%~dp0" || exit /b 1
where dotnet.exe >nul 2>nul
if errorlevel 1 (
  >&2 echo The .NET SDK is required. Install the SDK selected by global.json and retry.
  popd
  exit /b 127
)
dotnet --version >nul
if errorlevel 1 (
  >&2 echo A compatible .NET SDK is required. Check global.json and the installed SDKs.
  popd
  exit /b 1
)

:create_temp
set "dotask_bootstrap_dir=%TEMP%\dotask-bootstrap-%RANDOM%-%RANDOM%"
if exist "%dotask_bootstrap_dir%" goto create_temp
mkdir "%dotask_bootstrap_dir%" 2>nul
if errorlevel 1 (
  >&2 echo Could not create the temporary bootstrap directory.
  popd
  exit /b 1
)

rem MSBuild stages the evaluated publish output; no bin/obj paths are assumed.
dotnet publish src/Dotask.Cli/Dotask.Cli.csproj -c Release --nologo --verbosity minimal "-p:DotaskBootstrapDirectory=%dotask_bootstrap_dir%"
set "dotask_exit_code=%errorlevel%"
if not "%dotask_exit_code%"=="0" goto cleanup
if [%1]==[] goto default_target
dotnet "%dotask_bootstrap_dir%\dotask.dll" %*
set "dotask_exit_code=%errorlevel%"
goto cleanup

:default_target
dotnet "%dotask_bootstrap_dir%\dotask.dll" verify
set "dotask_exit_code=%errorlevel%"

:cleanup
rmdir /s /q "%dotask_bootstrap_dir%"
popd
exit /b %dotask_exit_code%
