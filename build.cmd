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

rem Stage the native runner, SDK, and evaluated C# support publish output.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".tasks\misc\prepare-bootstrap.ps1" -Destination "%dotask_bootstrap_dir%"
set "dotask_exit_code=%errorlevel%"
if not "%dotask_exit_code%"=="0" goto cleanup
if [%1]==[] goto default_target
"%dotask_bootstrap_dir%\dotask.exe" %*
set "dotask_exit_code=%errorlevel%"
goto cleanup

:default_target
"%dotask_bootstrap_dir%\dotask.exe" verify
set "dotask_exit_code=%errorlevel%"

:cleanup
rmdir /s /q "%dotask_bootstrap_dir%"
popd
exit /b %dotask_exit_code%
