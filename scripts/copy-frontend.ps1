# Copy frontend files to a dedicated folder for Tauri 2 build
# This avoids the "includes src-tauri/target/node_modules" error

$sourceDir = Split-Path -Parent $PSScriptRoot
$targetDir = Join-Path $sourceDir "frontend"

# Create frontend directory if not exists
if (-not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir | Out-Null
}

# Copy only the web files (no src-tauri, no node_modules)
Copy-Item -Path (Join-Path $sourceDir "index.html") -Destination $targetDir -Force
Copy-Item -Path (Join-Path $sourceDir "style.css") -Destination $targetDir -Force
Copy-Item -Path (Join-Path $sourceDir "app.js") -Destination $targetDir -Force
