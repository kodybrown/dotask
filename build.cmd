@echo off
setlocal DisableDelayedExpansion
pushd "%~dp0" || exit /b 1

rem Keep scratch under the cleanup-excluded Rust root, with a short Windows
rem prefix so deeply nested shared-task revision paths retain their path budget.
rem Nested launchers reuse this base rather than adding another nested prefix.
if not defined DOTASK_DEV_TEMP_ROOT set "DOTASK_DEV_TEMP_ROOT=%TEMP%\_rust"
if not exist "%DOTASK_DEV_TEMP_ROOT%" mkdir "%DOTASK_DEV_TEMP_ROOT%" 2>nul
if not exist "%DOTASK_DEV_TEMP_ROOT%" (
  >&2 echo Could not create the protected development temporary directory.
  popd
  exit /b 1
)
set "TEMP=%DOTASK_DEV_TEMP_ROOT%"
set "TMP=%DOTASK_DEV_TEMP_ROOT%"

:create_temp
set "dotask_bootstrap_dir=%TEMP%\dotask-bootstrap-%RANDOM%-%RANDOM%"
if exist "%dotask_bootstrap_dir%" goto create_temp
mkdir "%dotask_bootstrap_dir%" 2>nul
if errorlevel 1 (
  >&2 echo Could not create the temporary bootstrap directory.
  popd
  exit /b 1
)

rem Stage a physical native runner; task compilation prepares SDKs as needed.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File ".tasks\misc\prepare-bootstrap.ps1" -Destination "%dotask_bootstrap_dir%"
set "dotask_exit_code=%errorlevel%"
if not "%dotask_exit_code%"=="0" goto cleanup
set "DOTASK_SOURCE_RUST_SDK=%CD%\src\dotask-sdk"
set "DOTASK_SOURCE_ROOT=%CD%"
set "DOTASK_SOURCE_SDK_STAGE=%dotask_bootstrap_dir%\sdk"
"%dotask_bootstrap_dir%\dotask.exe" %*
set "dotask_exit_code=%errorlevel%"
goto cleanup

:cleanup
rmdir /s /q "%dotask_bootstrap_dir%"
popd
exit /b %dotask_exit_code%
