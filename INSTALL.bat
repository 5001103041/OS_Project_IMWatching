@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul
title IMWatching - Cài đặt Toàn bộ Thư viện & Môi trường
color 0B

echo ======================================================================
echo    CAI DAT TOAN BO CONG CU VA THU VIEN CHO DU AN IMWATCHING
echo ======================================================================
echo.

set "ROOT_DIR=%~dp0"
cd /d "%ROOT_DIR%"

REM 1. Kiem tra va ho tro tai cac cong cu he thong qua winget
echo [*] Kiem tra cac cong cu he thong (Rust, Node.js, FFmpeg)...

where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [!] Chua tim thay Rust. Dang tu dong tai va cai dat qua winget...
    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
) else (
    echo   -^> Rust/Cargo: Da co san.
)

where npm >nul 2>nul
if %errorlevel% neq 0 (
    echo [!] Chua tim thay Node.js. Dang tu dong tai va cai dat qua winget...
    winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements
) else (
    echo   -^> Node.js/npm: Da co san.
)

where ffmpeg >nul 2>nul
if %errorlevel% neq 0 (
    echo [!] Chua tim thay FFmpeg. Dang tu dong tai va cai dat qua winget...
    winget install --id Gyan.FFmpeg -e --accept-source-agreements --accept-package-agreements
) else (
    echo   -^> FFmpeg: Da co san.
)

REM 2. Cai dat thu vien Frontend
echo.
echo [*] Dang cai dat thu vien Frontend (npm install)...
cd /d "%ROOT_DIR%frontend"
call npm install
cd /d "%ROOT_DIR%"

REM 3. Bien dich va tai cac crates Backend
echo.
echo [*] Dang tai va bien dich cac goi Backend (cargo build)...
cd /d "%ROOT_DIR%backend"
call cargo build
cd /d "%ROOT_DIR%"

REM 4. Khoi tao .env neu chua co
if not exist "%ROOT_DIR%backend\.env" (
    if exist "%ROOT_DIR%backend\.env.example" (
        copy "%ROOT_DIR%backend\.env.example" "%ROOT_DIR%backend\.env" >nul
        echo   -^> Da khoi tao file backend\.env
    )
)
if not exist "%ROOT_DIR%backend\data" mkdir "%ROOT_DIR%backend\data"
if not exist "%ROOT_DIR%backend\media" mkdir "%ROOT_DIR%backend\media"

echo.
echo ======================================================================
echo   DA CAI DAT DAY DU TOAN BO MOI TRUONG VA THU VIEN!
echo   Ban co the nhap dup chuot vao START.bat de khoi dong he thong.
echo ======================================================================
echo.
pause
