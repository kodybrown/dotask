@echo off
setlocal DisableDelayedExpansion
pushd "%~dp0" || exit /b 1

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
set "DOTASK_SOURCE_SDK_STAGE=%dotask_bootstrap_dir%\sdk"
"%dotask_bootstrap_dir%\dotask.exe" %*
set "dotask_exit_code=%errorlevel%"
goto cleanup

:cleanup
rmdir /s /q "%dotask_bootstrap_dir%"
popd
exit /b %dotask_exit_code%
