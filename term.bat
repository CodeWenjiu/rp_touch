@echo off
echo Flashing device with picotool...
picotool load -u -v -x -t elf %1

timeout /t 2 /nobreak >nul

echo Connecting to serial port...
tio COM12 -m INLCRNL
