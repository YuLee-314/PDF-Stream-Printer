$response = Invoke-RestMethod -Uri "https://api.github.com/repos/bblanchon/pdfium-binaries/releases/latest"

$zip = $response.assets | Where-Object { $_.name -match "pdfium-win-x64" -and $_.name -notmatch "v8" } | Select-Object -First 1

if (-not $zip) {
    Write-Host "Not found. Visit: https://ghproxy.net/https://github.com/bblanchon/pdfium-binaries/releases"
    exit 1
}

$url = $zip.browser_download_url -replace "https://github.com", "https://ghproxy.net/https://github.com"
Write-Host "Downloading: $url"

$ext = if ($zip.name -match "\.tgz$") { "tgz" } else { "zip" }
Invoke-WebRequest -Uri $url -OutFile "pdfium.$ext"

Remove-Item -Recurse pdfium_temp -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory pdfium_temp -Force | Out-Null

if ($ext -eq "tgz") {
    tar -xzf "pdfium.$ext" -C pdfium_temp
} else {
    Expand-Archive -Path "pdfium.$ext" -DestinationPath pdfium_temp -Force
}

$dll = Get-ChildItem -Recurse pdfium_temp -Filter "pdfium.dll" | Select-Object -First 1
if ($dll) {
    Copy-Item $dll.FullName .\src-tauri\ -Force
    Write-Host "OK: pdfium.dll copied to src-tauri\"
} else {
    Write-Host "ERROR: pdfium.dll not found in archive"
    Get-ChildItem -Recurse pdfium_temp | Select-Object -First 30
}

Remove-Item -Recurse pdfium_temp -Force
Remove-Item "pdfium.$ext" -Force
