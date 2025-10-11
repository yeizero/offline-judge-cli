# Step 1 & 2: Ensure the current directory is 'scripts'; otherwise, exit with an error
$currentDir = Get-Location
$scriptsDir = Join-Path -Path $currentDir -ChildPath "scripts"

if ($currentDir.BaseName -eq "scripts") {
    Set-Location ..
} elseif (Test-Path $scriptsDir) {
    # Already not in 'scripts', but 'scripts' folder exists, continue
    Set-Location $currentDir
} else {
    Write-Error "No 'scripts' folder found in the current directory. Exiting script."
    exit 1
}

# Step 3: Build the project in release mode
Write-Host "Building project..."
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed."
    exit 1
}

# Step 4: Copy and rename files to target/pack
$sourceDir = Join-Path -Path (Get-Location) -ChildPath "target\release"
$destDir = Join-Path -Path (Get-Location) -ChildPath "target\pack"

# Create destination folder if it doesn't exist
if (-not (Test-Path $destDir)) {
    New-Item -ItemType Directory -Path $destDir | Out-Null
}

# Define the files to copy and their new names
$filesToCopy = @{
    "editor.exe"       = "editor.exe"
    "oj-evaluator.exe" = "judge.exe"
    "oj-generator.exe" = "judgeg.exe"
}

foreach ($sourceFile in $filesToCopy.Keys) {
    $sourcePath = Join-Path -Path $sourceDir -ChildPath $sourceFile
    $destPath   = Join-Path -Path $destDir -ChildPath $filesToCopy[$sourceFile]

    if (Test-Path $sourcePath) {
        Copy-Item -Path $sourcePath -Destination $destPath -Force
        Write-Host "Copied $sourceFile -> $($filesToCopy[$sourceFile])"
    } else {
        Write-Warning "Source file '$sourceFile' not found. Skipping."
    }
}

Write-Host "All done!"