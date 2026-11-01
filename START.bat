@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul
title IMWatching - Khởi động Hệ thống
color 0A

echo ======================================================================
echo    HE THONG PHAN TICH VIDEO AI & GIAM SAT HE DIEU HANH - IMWATCHING
echo ======================================================================
echo.

REM 1. Lay duong dan thu muc goc chua file .bat
set "ROOT_DIR=%~dp0"
cd /d "%ROOT_DIR%"

REM 2. Kiem tra Rust / Cargo
echo [*] Dang kiem tra moi truong Rust (Cargo)...
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    color 0C
    echo [LOI] Khong tim thay Rust / Cargo tren may tinh cua ban.
    echo Vui long cai dat Rust tai: https://rustup.rs
    echo Sau khi cai dat, vui long khoi dong lai Command Prompt hoac may tinh.
    echo.
    pause
    exit /b 1
)
echo   -^> Rust/Cargo: SAN SANG.

REM 3. Kiem tra Node.js / npm
echo [*] Dang kiem tra moi truong Node.js (npm)...
where npm >nul 2>nul
if %errorlevel% neq 0 (
    color 0C
    echo [LOI] Khong tim thay Node.js / npm tren may tinh cua ban.
    echo Vui long cai dat Node.js tai: https://nodejs.org
    echo.
    pause
    exit /b 1
)
echo   -^> Node.js/npm: SAN SANG.

REM 4. Kiem tra FFmpeg
echo [*] Dang kiem tra cong cu FFmpeg...
where ffmpeg >nul 2>nul
if %errorlevel% neq 0 (
    echo   -^> [CANH BAO] Khong tim thay FFmpeg trong bien moi truong PATH.
    echo      Tinh nang tach khung hinh video co the can FFmpeg de hoat dong.
    echo      (Ban co the cai dat FFmpeg tai: https://ffmpeg.org/download.html)
    echo.
) else (
    echo   -^> FFmpeg: SAN SANG.
)

REM 5. Kiem tra va khoi tao file cau hinh Backend (.env)
if not exist "%ROOT_DIR%backend\.env" (
    echo [*] Chua co file backend\.env, dang tu dong khoi tao tu backend\.env.example...
    if exist "%ROOT_DIR%backend\.env.example" (
        copy "%ROOT_DIR%backend\.env.example" "%ROOT_DIR%backend\.env" >nul
        echo   -^> Da tao file backend\.env thanh cong.
    ) else (
        echo PORT=8080> "%ROOT_DIR%backend\.env"
        echo DATABASE_URL=sqlite://./data/imwatching.db?mode=rwc>> "%ROOT_DIR%backend\.env"
        echo JWT_SECRET=imwatching-default-jwt-secret-key-at-least-32-chars-long>> "%ROOT_DIR%backend\.env"
        echo MEDIA_DIR=./media>> "%ROOT_DIR%backend\.env"
        echo AI_ENABLED=false>> "%ROOT_DIR%backend\.env"
        echo FFMPEG_BIN=ffmpeg>> "%ROOT_DIR%backend\.env"
        echo FFPROBE_BIN=ffprobe>> "%ROOT_DIR%backend\.env"
        echo FRAME_FPS=1>> "%ROOT_DIR%backend\.env"
        echo   -^> Da tao file backend\.env mac dinh.
    )
)

REM Tao cac thu muc can thiet neu chua co
if not exist "%ROOT_DIR%backend\data" mkdir "%ROOT_DIR%backend\data"
if not exist "%ROOT_DIR%backend\media" mkdir "%ROOT_DIR%backend\media"

REM 6. Kiem tra node_modules cua Frontend (tu dong cai neu nguoi khac moi tai ve)
if not exist "%ROOT_DIR%frontend\node_modules" (
    echo.
    echo [*] Phat hien Frontend chua co thu vien node_modules.
    echo [*] Dang tu dong chay 'npm install' cho Frontend, vui long cho trong giay lat...
    cd /d "%ROOT_DIR%frontend"
    call npm install
    if %errorlevel% neq 0 (
        color 0C
        echo [LOI] Khong the cai dat thu vien Frontend qua npm install.
        pause
        exit /b 1
    )
    cd /d "%ROOT_DIR%"
    echo   -^> Cai dat thu vien Frontend thanh cong.
)

echo.
echo ======================================================================
echo   DANG KHOI CHAY DONG THOI BACKEND VA FRONTEND...
echo ======================================================================
echo.

REM 7. Khoi dong Backend trong cua so rieng
echo [*] Dang khoi dong Backend Server tai port 8080...
start "IMWatching - Backend (Port 8080)" cmd /k "cd /d "%ROOT_DIR%backend" && cargo run"

REM Cho 3 giay de Backend khoi tao DB va lang nghe port
timeout /t 3 /nobreak >nul

REM 8. Khoi dong Frontend trong cua so rieng
echo [*] Dang khoi dong Frontend Web Server tai port 5173...
start "IMWatching - Frontend (Port 5173)" cmd /k "cd /d "%ROOT_DIR%frontend" && npm run dev"

REM Cho 2 giay de Vite san sang
timeout /t 2 /nobreak >nul

REM 9. Tu dong mo trinh duyet Web
echo [*] Dang tu dong mo trinh duyet web...
start http://localhost:5173

echo.
echo ======================================================================
echo   HE THONG DA KHOI DONG THANH CONG!
echo ======================================================================
echo   - Giao dien Web:        http://localhost:5173
echo   - Backend REST API:     http://localhost:8080
echo   - API Telemetry HDH:    http://localhost:8080/api/os/metrics
echo.
echo   Luu y: Vui long giu 2 cua so Backend va Frontend mo trong khi su dung.
echo   De tat he thong, ban co the chay file STOP.bat hoac tat cac cua so.
echo ======================================================================
echo.
pause
