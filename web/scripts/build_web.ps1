$ErrorActionPreference = "Stop"

$OutDir = "web/Survey_web"

Write-Host "Building WASM..."
cargo build --release --target wasm32-unknown-unknown

Write-Host "Packaging..."
if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Force $OutDir | Out-Null
Copy-Item "target/wasm32-unknown-unknown/release/Survey.wasm" "$OutDir/survey.wasm"
Copy-Item "web/bundle/index.html" "$OutDir/"
Copy-Item "web/bundle/mq_js_bundle.js" "$OutDir/"

Write-Host "Zipping..."
if (Test-Path "web/Survey_web.zip") { Remove-Item "web/Survey_web.zip" }
$files = Get-ChildItem -Path $OutDir -File | Select-Object -ExpandProperty FullName
Compress-Archive -Path $files -DestinationPath "web/Survey_web.zip"

Write-Host "Done -> web/Survey_web.zip"
