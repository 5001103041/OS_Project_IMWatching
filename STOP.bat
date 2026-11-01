@echo off
chcp 65001 >nul
title IMWatching - Dừng Hệ thống
color 0E

echo ======================================================================
echo         DANG DUNG TOAN BO HE THONG IMWATCHING...
echo ======================================================================
echo.

echo [*] Dang dong tien trinh Backend (imwatching-backend.exe)...
taskkill /F /IM imwatching-backend.exe /T >nul 2>nul
taskkill /F /IM cargo.exe /FI "WINDOWTITLE eq IMWatching - Backend*" >nul 2>nul

echo [*] Dang dong tien trinh Frontend (Vite / Node.js)...
taskkill /F /FI "WINDOWTITLE eq IMWatching - Frontend*" >nul 2>nul

REM Tim va dong tien trinh tren port 8080 va 5173 neu con sot lai
for /f "tokens=5" %%a in ('netstat -aon 2^>nul ^| findstr :8080 ^| findstr LISTENING') do (
    taskkill /F /PID %%a >nul 2>nul
)
for /f "tokens=5" %%a in ('netstat -aon 2^>nul ^| findstr :5173 ^| findstr LISTENING') do (
    taskkill /F /PID %%a >nul 2>nul
)

echo.
echo [V] Da tat toan bo Backend va Frontend thanh cong!
echo ======================================================================
echo.
pause
