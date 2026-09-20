@echo off
rem Yaca in dev: vite, the core and the window in one command.
rem
rem Lives at the repo root and switches to its own directory, so a double click
rem works from anywhere. Every failure ends in `pause`: a console window that
rem closes on its own explains nothing to the person who double-clicked it.
setlocal

rem Messages below are Russian, and cmd reads this file in the console code
rem page. Without UTF-8 they come out as mojibake; the previous page is put
rem back at the end so the console is left as it was found.
for /f "tokens=2 delims=:." %%p in ('chcp') do set "YACA_CP=%%p"
chcp 65001 >nul

cd /d "%~dp0"

where node >nul 2>nul
if errorlevel 1 (
    echo Не найден node. Нужен Node.js 20 или новее: https://nodejs.org
    goto :fail
)

where cargo >nul 2>nul
if errorlevel 1 (
    echo Не найден cargo. Нужен Rust: https://rustup.rs
    goto :fail
)

if not exist "node_modules" (
    echo Ставлю зависимости — это один раз.
    call npm install
    if errorlevel 1 (
        echo Не удалось поставить зависимости.
        goto :fail
    )
)

echo Собираю ядро и поднимаю окно. Первая сборка долгая: cargo собирает всё.
echo.
if "%~1"=="" (
    call npm run tauri dev
) else (
    call npm run tauri dev -- %*
)
if errorlevel 1 goto :fail

chcp %YACA_CP% >nul
endlocal
exit /b 0

:fail
echo.
pause
chcp %YACA_CP% >nul
endlocal
exit /b 1
