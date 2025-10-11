#!/usr/bin/env bash
set -e  # Exit immediately if a command exits with a non-zero status

# Step 1 & 2: Ensure the current directory is 'scripts'; otherwise, exit
currentDir=$(basename "$PWD")
if [[ "$currentDir" == "scripts" ]]; then
    cd ..
elif [[ -d "./scripts" ]]; then
    # Already not in 'scripts', but 'scripts' folder exists, continue
    :
else
    echo "Error: No 'scripts' folder found in the current directory. Exiting script." >&2
    exit 1
fi

# Step 3: Build the project in release mode
echo "Building project..."
cargo build --release

# Step 4: Copy and rename files to target/pack
sourceDir="./target/release"
destDir="./target/packed"

# Create destination folder if it doesn't exist
mkdir -p "$destDir"

# Define files to copy and their new names
declare -A filesToCopy=(
    ["editor"]="editor"
    ["oj-evaluator"]="judge"
    ["oj-generator"]="judgeg"
)

for src in "${!filesToCopy[@]}"; do
    srcPath="$sourceDir/$src"
    destPath="$destDir/${filesToCopy[$src]}"

    if [[ -f "$srcPath" ]]; then
        cp "$srcPath" "$destPath"
        echo "Copied $src -> ${filesToCopy[$src]}"
    else
        echo "Warning: Source file '$src' not found. Skipping." >&2
    fi
done

configSrc="./scripts/default_config.yaml"
configDest="$destDir/config.yaml"

if [[ -f "$configSrc" ]]; then
    cp "$configSrc" "$configDest"
    echo "Copied default_config.yaml -> config.yaml"
else
    echo "Warning: default_config.yaml not found in scripts. Skipping." >&2
fi

echo "All done!"
