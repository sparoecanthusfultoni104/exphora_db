@echo off
echo Cerrando instancias previas...
taskkill /F /IM exphora_db.exe 2>nul
timeout /t 1 /nobreak >nul
echo Compilando...
cargo build --release
if %ERRORLEVEL% == 0 (
    echo.
    echo BUILD EXITOSO — target/release/exphora_db.exe
    echo Iniciando aplicacion...
    start "" "target\release\exphora_db.exe"
) else (
    echo BUILD FALLIDO
)
pause
