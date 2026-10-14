@echo off
cd /d "%~dp0"
echo Downloading FFmpeg, FFprobe, yt-dlp and Deno. Internet access is required.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\install-media.ps1" -RuntimeDir "%~dp0tools"
if errorlevel 1 (
    echo Media tools could not be installed. Check the error above and retry.
    pause
    exit /b 1
)
echo Media tools installed. Restart GenexDome to use them.
pause
