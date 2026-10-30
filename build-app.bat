@echo off
setlocal
title Spine - building the app
set "HERE=%~dp0"
set "LOG=%HERE%build-app-log.txt"
set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
set "RUSTUP=%USERPROFILE%\.cargo\bin\rustup.exe"
set "TC=stable-x86_64-pc-windows-msvc"

echo Spine app build log > "%LOG%"
echo %date% %time% >> "%LOG%"
if not exist "%CARGO%" (
  echo === CARGO_NOT_FOUND === >> "%LOG%"
  echo Rust isn't installed. Run build-and-scan.bat first.
  pause
  exit /b 1
)

echo.
echo Getting Rust's Visual Studio toolchain (one time)...
"%RUSTUP%" toolchain install %TC% --profile minimal >> "%LOG%" 2>&1

echo Building Spine. The first build takes a few minutes; after that it's quick.
echo === BUILD === >> "%LOG%"
cd /d "%HERE%spine-app"
"%CARGO%" +%TC% build --release >> "%LOG%" 2>&1
if errorlevel 1 goto failed

rem Close Spine if it is open so the new version can replace it.
taskkill /im Spine.exe /f >nul 2>&1
timeout /t 1 /nobreak >nul
copy /y "target\release\spine.exe" "%HERE%Spine.exe" >nul
echo === BUILD_OK === >> "%LOG%"

rem Desktop shortcut (works with OneDrive desktops too).
powershell -NoProfile -ExecutionPolicy Bypass -Command "$d=[Environment]::GetFolderPath('Desktop'); $s=(New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $d 'Spine.lnk')); $s.TargetPath='%HERE%Spine.exe'; $s.WorkingDirectory='%HERE%'; $s.IconLocation='%HERE%Spine.exe,0'; $s.Save()" >> "%LOG%" 2>&1
echo === SHORTCUT_DONE === >> "%LOG%"

start "" "%HERE%Spine.exe"
exit /b 0

:failed
echo === BUILD_FAILED === >> "%LOG%"
echo.
echo The build failed. The details are in build-app-log.txt - Claude can read it from there.
pause
exit /b 1
