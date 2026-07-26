[CmdletBinding()]
param(
    [string[]]$MessageCounts = @('50', '500', '2000'),
    [string[]]$LocaleCounts = @('1', '5', '20'),
    [ValidateRange(1, 20)]
    [int]$Repetitions = 3,
    [switch]$SmokeOnly,
    [string[]]$ProfileCases = @('50x1', '500x5', '2000x20')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script:MacroRoot = Join-Path $script:RepositoryRoot 'i18n-macro'
$script:FixtureRoot = Join-Path $script:RepositoryRoot 'target\i18n-perf\fixture'
$script:CargoTargetRoot = Join-Path $script:RepositoryRoot 'target\i18n-perf\cargo-target'
$script:ResultsRoot = Join-Path $script:RepositoryRoot 'target\i18n-perf\results'
$script:MacroPathForToml = $script:MacroRoot.Replace('\', '/')

function ConvertTo-BenchmarkIntegers {
    [OutputType([int[]])]
    param(
        [string[]]$Values,
        [string]$ParameterName,
        [int]$Minimum
    )

    $numbers = [System.Collections.Generic.List[int]]::new()
    foreach ($value in $Values) {
        foreach ($part in $value.Split(',')) {
            $trimmed = $part.Trim()
            $number = 0
            if ([string]::IsNullOrWhiteSpace($trimmed) -or -not [int]::TryParse(
                    $trimmed,
                    [System.Globalization.NumberStyles]::Integer,
                    [System.Globalization.CultureInfo]::InvariantCulture,
                    [ref]$number
                )) {
                throw "$ParameterName must contain comma-delimited integers; received '$value'."
            }
            if ($number -lt $Minimum) {
                throw "$ParameterName must be at least $Minimum; received $number."
            }
            $numbers.Add($number)
        }
    }
    if ($numbers.Count -eq 0) {
        throw "$ParameterName must contain at least one integer."
    }
    return $numbers.ToArray()
}

$script:NormalizedMessageCounts = ConvertTo-BenchmarkIntegers -Values $MessageCounts -ParameterName 'MessageCounts' -Minimum 4
$script:NormalizedLocaleCounts = ConvertTo-BenchmarkIntegers -Values $LocaleCounts -ParameterName 'LocaleCounts' -Minimum 1

function ConvertTo-ProfileCaseList {
    [OutputType([object[]])]
    param([string[]]$Values)

    $allowed = @{
        '50x1' = $true
        '500x5' = $true
        '2000x20' = $true
    }
    $cases = [System.Collections.Generic.List[object]]::new()
    foreach ($value in $Values) {
        foreach ($part in $value.Split(',')) {
            $trimmed = $part.Trim()
            if ($trimmed -notmatch '^(?<messages>\d+)x(?<locales>\d+)$') {
                throw "ProfileCases must use <messages>x<locales>; received '$part'."
            }
            $caseName = '{0}x{1}' -f $Matches.messages, $Matches.locales
            if (-not $allowed.ContainsKey($caseName)) {
                throw "ProfileCases supports only 50x1, 500x5, and 2000x20; received '$caseName'."
            }
            $cases.Add([pscustomobject]@{
                MessageCount = [int]$Matches.messages
                LocaleCount = [int]$Matches.locales
                Name = $caseName
            })
        }
    }
    if ($cases.Count -eq 0) {
        throw 'ProfileCases must contain at least one representative tuple.'
    }
    return $cases.ToArray()
}

$script:NormalizedProfileCases = ConvertTo-ProfileCaseList -Values $ProfileCases
if (-not (Test-Path (Join-Path $script:MacroRoot 'Cargo.toml') -PathType Leaf)) {
    throw "Missing i18n-macro manifest at $script:MacroRoot."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo is required to run the Nightly fixture.'
}
& cargo +nightly --version *> $null
if ($LASTEXITCODE -ne 0) {
    throw 'The cargo +nightly toolchain is required.'
}

function New-DefaultCatalog {
    [OutputType([string])]
    param([int]$MessageCount)

    $lines = [System.Collections.Generic.List[string]]::new()
    for ($index = 0; $index -lt $MessageCount; $index++) {
        $key = 'Message{0:D4}' -f $index
        switch ($index % 4) {
            0 { $lines.Add("    $key = `"default static {0:D4}`";" -f $index) }
            1 { $lines.Add("    $key {{}} = `"default zero {0:D4}`";" -f $index) }
            2 { $lines.Add("    $key { first, second } = `"{first}:{second}`";") }
            3 { $lines.Add("    $key { first: usize, second: usize } = `"{first}:{second}`";") }
        }
    }
    return ($lines -join "`n")
}

function New-LocaleCatalog {
    [OutputType([string])]
    param(
        [int]$MessageCount,
        [int]$LocaleIndex,
        [int]$Revision
    )

    $lines = [System.Collections.Generic.List[string]]::new()
    for ($index = 0; $index -lt $MessageCount; $index++) {
        $key = 'Message{0:D4}' -f $index
        $revisionSuffix = if ($Revision -ge 0 -and $index -eq 0) {
            " BENCH_REVISION_$Revision"
        } else {
            ''
        }
        switch ($index % 4) {
            0 { $lines.Add(("    $key = `"locale {0:D3} static {1:D4}$revisionSuffix`";" -f $LocaleIndex, $index)) }
            1 { $lines.Add(("    $key {{}} = `"locale {0:D3} zero {1:D4}`";" -f $LocaleIndex, $index)) }
            2 {
                if (([math]::Floor($index / 4) % 2) -eq 0) {
                    $lines.Add("    $key { first, second } = `"{first}:{second}`";")
                } else {
                    $lines.Add("    $key { first, .. } = `"{first}`";")
                }
            }
            3 {
                if (([math]::Floor($index / 4) % 2) -eq 0) {
                    $lines.Add("    $key { first: usize, second: usize } = `"{first}:{second}`";")
                } else {
                    $lines.Add("    $key { first: usize, .. } = `"{first}`";")
                }
            }
        }
    }
    return ($lines -join "`n")
}

function New-FixtureSource {
    [OutputType([string])]
    param(
        [int]$MessageCount,
        [int]$LocaleCount,
        [int]$Revision
    )

    $defaultCatalog = New-DefaultCatalog -MessageCount $MessageCount
    if ($LocaleCount -eq 1) {
        $defaultCatalog = $defaultCatalog.Replace(
            'default static 0000',
            "default static 0000 BENCH_REVISION_$Revision"
        )
    }

    $localeModules = [System.Collections.Generic.List[string]]::new()
    $localeEntries = [System.Collections.Generic.List[string]]::new()
    for ($localeIndex = 1; $localeIndex -lt $LocaleCount; $localeIndex++) {
        $catalogRevision = if ($localeIndex -eq ($LocaleCount - 1)) { $Revision } else { -1 }
        $catalog = New-LocaleCatalog -MessageCount $MessageCount -LocaleIndex $localeIndex -Revision $catalogRevision
        $localeModules.Add((@"
mod locale_{0:D3} {{
    use super::catalog;

    catalog! {{
        schema: super::en_us;

{1}
    }}
}}
"@ -f $localeIndex, $catalog))
        $localeEntries.Add("    locale Locale{0:D3}: locale_{0:D3};" -f $localeIndex)
    }

    $calls = [System.Collections.Generic.List[string]]::new()
    foreach ($index in 0..3) {
        $key = 'Message{0:D4}' -f $index
        switch ($index % 4) {
            0 {
                $calls.Add("    let _ = tr!($key);")
                $calls.Add("    let _ = tr_for!(Locale::EnUs, $key);")
            }
            1 {
                $calls.Add(('    let _ = format!("{{}}", tr!({0} {{}}));' -f $key))
                $calls.Add(('    let _ = format!("{{}}", tr_for!(Locale::EnUs, {0} {{}}));' -f $key))
            }
            2 {
                $calls.Add(('    let _ = format!("{{}}", tr!({0} {{ first: "first", second: "second" }}));' -f $key))
                $calls.Add(('    let _ = format!("{{}}", tr_for!(Locale::EnUs, {0} {{ first: "first", second: "second" }}));' -f $key))
            }
            3 {
                $calls.Add(('    let _ = format!("{{}}", tr!({0} {{ first: 1usize, second: 2usize }}));' -f $key))
                $calls.Add(('    let _ = format!("{{}}", tr_for!(Locale::EnUs, {0} {{ first: 1usize, second: 2usize }}));' -f $key))
            }
        }
    }

    return @"
use i18n_macro::{catalog, define_i18n};

mod en_us {
    use super::catalog;

    catalog! {
        default;

$defaultCatalog
    }
}

$($localeModules -join "`n")
fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
$($localeEntries -join "`n")
}

fn main() {
$($calls -join "`n")
}
"@
}

function Write-Fixture {
    param(
        [int]$MessageCount,
        [int]$LocaleCount,
        [int]$Revision
    )

    $sourceDirectory = Join-Path $script:FixtureRoot 'src'
    New-Item -ItemType Directory -Force -Path $sourceDirectory, $script:CargoTargetRoot, $script:ResultsRoot | Out-Null

    $manifest = @"
[package]
name = "i18n-perf-fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[dependencies]
i18n-macro = { path = "$script:MacroPathForToml" }
"@
    $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText((Join-Path $script:FixtureRoot 'Cargo.toml'), $manifest, $utf8WithoutBom)
    [System.IO.File]::WriteAllText(
        (Join-Path $sourceDirectory 'main.rs'),
        (New-FixtureSource -MessageCount $MessageCount -LocaleCount $LocaleCount -Revision $Revision),
        $utf8WithoutBom
    )
}

function Invoke-Checked {
    [OutputType([pscustomobject])]
    param(
        [string]$Executable,
        [string[]]$Arguments,
        [string]$WorkingDirectory,
        [hashtable]$Environment = @{}
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Executable
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        [void]$startInfo.ArgumentList.Add($argument)
    }
    foreach ($key in $Environment.Keys) {
        $startInfo.Environment[$key] = [string]$Environment[$key]
    }

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    [void]$process.Start()
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $stopwatch.Stop()
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $result = [pscustomobject]@{
        ExitCode = $process.ExitCode
        ElapsedMilliseconds = $stopwatch.ElapsedMilliseconds
        Stdout = $stdout
        Stderr = $stderr
    }
    if ($result.ExitCode -ne 0) {
        $command = "$Executable $($Arguments -join ' ')"
        throw "Command failed: $command`nExit code: $($result.ExitCode)`nStdout:`n$($result.Stdout)`nStderr:`n$($result.Stderr)"
    }
    return $result
}

function Get-BenchmarkEnvironment {
    return @{
        CARGO_TARGET_DIR = $script:CargoTargetRoot
        RUSTFLAGS = '-Zthreads=1'
    }
}

function Get-SourceMetrics {
    $sourcePath = Join-Path $script:FixtureRoot 'src\main.rs'
    return [pscustomobject]@{
        Bytes = ([System.IO.FileInfo]$sourcePath).Length
        Lines = ([System.IO.File]::ReadAllLines($sourcePath)).Length
    }
}

function New-TimingRow {
    [OutputType([pscustomobject])]
    param(
        [int]$MessageCount,
        [int]$LocaleCount,
        [int]$Cycle,
        [ValidateSet('cold', 'warm', 'incremental')]
        [string]$Kind,
        [pscustomobject]$Result,
        [pscustomobject]$SourceMetrics
    )

    return [pscustomobject][ordered]@{
        timestamp_utc = [DateTime]::UtcNow.ToString('o', [System.Globalization.CultureInfo]::InvariantCulture)
        commit = $script:BenchmarkCommit
        rustc_version = $script:RustcVersion
        message_count = $MessageCount
        locale_count = $LocaleCount
        cycle = $Cycle
        kind = $Kind
        elapsed_ms = $Result.ElapsedMilliseconds
        source_bytes = $SourceMetrics.Bytes
        source_lines = $SourceMetrics.Lines
        exit_code = $Result.ExitCode
    }
}

function Get-Summary {
    [OutputType([object[]])]
    param([object[]]$Rows)

    $summaries = [System.Collections.Generic.List[object]]::new()
    foreach ($group in ($Rows | Group-Object { "$($_.message_count)|$($_.locale_count)|$($_.kind)" })) {
        $sample = $group.Group[0]
        $values = @($group.Group | ForEach-Object { [double]$_.elapsed_ms } | Sort-Object)
        $middle = [int][math]::Floor($values.Count / 2)
        $median = if (($values.Count % 2) -eq 1) {
            $values[$middle]
        } else {
            ($values[$middle - 1] + $values[$middle]) / 2
        }
        $summaries.Add([pscustomobject][ordered]@{
            message_count = $sample.message_count
            locale_count = $sample.locale_count
            kind = $sample.kind
            min_ms = $values[0]
            median_ms = $median
            max_ms = $values[$values.Count - 1]
        })
    }
    return @($summaries | Sort-Object message_count, locale_count, kind)
}

function ConvertTo-InvariantCsvField {
    [OutputType([string])]
    param([object]$Value)

    if ($null -eq $Value) {
        return ''
    }
    $text = if ($Value -is [System.IFormattable]) {
        $Value.ToString($null, [System.Globalization.CultureInfo]::InvariantCulture)
    } else {
        [string]$Value
    }
    if ($text.IndexOfAny([char[]]@(',', '"', "`r", "`n")) -ge 0) {
        return '"' + $text.Replace('"', '""') + '"'
    }
    return $text
}

function Write-BenchmarkCsv {
    param(
        [object[]]$Rows,
        [string[]]$Columns,
        [string]$Path
    )

    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add(($Columns -join ','))
    foreach ($row in $Rows) {
        $fields = foreach ($column in $Columns) {
            ConvertTo-InvariantCsvField -Value $row.$column
        }
        $lines.Add(($fields -join ','))
    }
    [System.IO.File]::WriteAllLines($Path, $lines, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-DependencyPrewarm {
    Write-Fixture -MessageCount 4 -LocaleCount 1 -Revision 0
    $environment = Get-BenchmarkEnvironment
    [void](Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'generate-lockfile', '--offline') -WorkingDirectory $script:FixtureRoot -Environment $environment)
    $check = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'check', '--offline', '--locked') -WorkingDirectory $script:FixtureRoot -Environment $environment
    Write-Output "PREWARM elapsed_ms=$($check.ElapsedMilliseconds)"
}

function Invoke-TimingCycle {
    [OutputType([object[]])]
    param(
        [int]$MessageCount,
        [int]$LocaleCount,
        [int]$Cycle
    )

    $manifestPath = Join-Path $script:FixtureRoot 'Cargo.toml'
    $environment = Get-BenchmarkEnvironment
    $rows = [System.Collections.Generic.List[object]]::new()
    Write-Fixture -MessageCount $MessageCount -LocaleCount $LocaleCount -Revision 0
    $sourceMetrics = Get-SourceMetrics
    try {
        [void](Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'clean', '--manifest-path', $manifestPath, '-p', 'i18n-perf-fixture') -WorkingDirectory $script:FixtureRoot -Environment $environment)

        Write-Host "TIMING messages=$MessageCount locales=$LocaleCount cycle=$Cycle kind=cold"
        $cold = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'check', '--manifest-path', $manifestPath, '--offline', '--locked') -WorkingDirectory $script:FixtureRoot -Environment $environment
        $rows.Add((New-TimingRow -MessageCount $MessageCount -LocaleCount $LocaleCount -Cycle $Cycle -Kind cold -Result $cold -SourceMetrics $sourceMetrics))

        Write-Host "TIMING messages=$MessageCount locales=$LocaleCount cycle=$Cycle kind=warm"
        $warm = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'check', '--manifest-path', $manifestPath, '--offline', '--locked') -WorkingDirectory $script:FixtureRoot -Environment $environment
        $rows.Add((New-TimingRow -MessageCount $MessageCount -LocaleCount $LocaleCount -Cycle $Cycle -Kind warm -Result $warm -SourceMetrics $sourceMetrics))

        Write-Fixture -MessageCount $MessageCount -LocaleCount $LocaleCount -Revision 1
        Write-Host "TIMING messages=$MessageCount locales=$LocaleCount cycle=$Cycle kind=incremental"
        $incremental = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'check', '--manifest-path', $manifestPath, '--offline', '--locked') -WorkingDirectory $script:FixtureRoot -Environment $environment
        $rows.Add((New-TimingRow -MessageCount $MessageCount -LocaleCount $LocaleCount -Cycle $Cycle -Kind incremental -Result $incremental -SourceMetrics $sourceMetrics))
    } finally {
        Write-Fixture -MessageCount $MessageCount -LocaleCount $LocaleCount -Revision 0
    }
    return $rows.ToArray()
}

function Invoke-TimingMatrix {
    $script:BenchmarkCommit = (Invoke-Checked -Executable 'git' -Arguments @('-C', $script:RepositoryRoot, 'rev-parse', 'HEAD') -WorkingDirectory $script:RepositoryRoot).Stdout.Trim()
    $script:RustcVersion = (Invoke-Checked -Executable 'rustc' -Arguments @('+nightly', '--version') -WorkingDirectory $script:RepositoryRoot).Stdout.Trim()
    Invoke-DependencyPrewarm

    $rows = [System.Collections.Generic.List[object]]::new()
    foreach ($messageCount in @($script:NormalizedMessageCounts | Sort-Object)) {
        foreach ($localeCount in @($script:NormalizedLocaleCounts | Sort-Object)) {
            for ($cycle = 1; $cycle -le $Repetitions; $cycle++) {
                foreach ($row in (Invoke-TimingCycle -MessageCount $messageCount -LocaleCount $localeCount -Cycle $cycle)) {
                    $rows.Add($row)
                }
            }
        }
    }

    $rawPath = Join-Path $script:ResultsRoot 'timings.csv'
    $summaryPath = Join-Path $script:ResultsRoot 'timing-summary.csv'
    $rawColumns = @('timestamp_utc', 'commit', 'rustc_version', 'message_count', 'locale_count', 'cycle', 'kind', 'elapsed_ms', 'source_bytes', 'source_lines', 'exit_code')
    Write-BenchmarkCsv -Rows $rows.ToArray() -Columns $rawColumns -Path $rawPath
    $summary = Get-Summary -Rows $rows.ToArray()
    Write-BenchmarkCsv -Rows $summary -Columns @('message_count', 'locale_count', 'kind', 'min_ms', 'median_ms', 'max_ms') -Path $summaryPath
    Write-Output "TIMING COMPLETE rows=$($rows.Count) raw=$rawPath summary=$summaryPath"
}

function Write-Utf8File {
    param(
        [string]$Path,
        [string]$Contents
    )

    [System.IO.File]::WriteAllText($Path, $Contents, [System.Text.UTF8Encoding]::new($false))
}

function Get-FileMetrics {
    [OutputType([pscustomobject])]
    param([string]$Path)

    return [pscustomobject]@{
        Bytes = ([System.IO.FileInfo]$Path).Length
        Lines = ([System.IO.File]::ReadAllLines($Path)).Length
    }
}

function Invoke-OptionalMeasureme {
    param(
        [string]$CaseDirectory,
        [System.IO.FileInfo[]]$SelfProfileFiles
    )

    $summarize = Get-Command summarize -ErrorAction SilentlyContinue
    $crox = Get-Command crox -ErrorAction SilentlyContinue
    if ($null -eq $summarize -or $null -eq $crox) {
        Write-Utf8File -Path (Join-Path $CaseDirectory 'measureme-tools-unavailable.txt') -Contents "summarize_available=$($null -ne $summarize)`ncrox_available=$($null -ne $crox)`n"
        return
    }

    $index = 0
    foreach ($selfProfileFile in $SelfProfileFiles) {
        $index++
        $summary = Invoke-Checked -Executable $summarize.Source -Arguments @('summarize', $selfProfileFile.FullName) -WorkingDirectory $CaseDirectory
        Write-Utf8File -Path (Join-Path $CaseDirectory "measureme-summary-$index.txt") -Contents ($summary.Stdout + $summary.Stderr)
        $croxOutput = Join-Path $CaseDirectory 'chrome_profiler.json'
        if (Test-Path $croxOutput) {
            Remove-Item -LiteralPath $croxOutput -Force
        }
        $trace = Invoke-Checked -Executable $crox.Source -Arguments @($selfProfileFile.FullName) -WorkingDirectory $CaseDirectory
        if (-not (Test-Path $croxOutput) -or ([System.IO.FileInfo]$croxOutput).Length -eq 0) {
            throw "crox did not create a nonempty chrome_profiler.json for $($selfProfileFile.FullName)."
        }
        Move-Item -LiteralPath $croxOutput -Destination (Join-Path $CaseDirectory "measureme-trace-$index.json")
        Write-Utf8File -Path (Join-Path $CaseDirectory "measureme-trace-$index.stderr.txt") -Contents $trace.Stderr
    }
}

function Get-RegexCount {
    [OutputType([int])]
    param(
        [string]$Text,
        [string]$Pattern
    )

    return [regex]::Matches($Text, $Pattern).Count
}

function Invoke-ExpansionCollection {
    [OutputType([pscustomobject])]
    param(
        [int]$MessageCount,
        [int]$LocaleCount,
        [string]$CaseDirectory,
        [pscustomobject]$SourceMetrics
    )

    $manifestPath = Join-Path $script:FixtureRoot 'Cargo.toml'
    $expansion = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'expand', '--manifest-path', $manifestPath, '--bin', 'i18n-perf-fixture') -WorkingDirectory $script:FixtureRoot -Environment (Get-BenchmarkEnvironment)
    $expandedPath = Join-Path $CaseDirectory 'expanded.rs'
    Write-Utf8File -Path $expandedPath -Contents $expansion.Stdout
    Write-Utf8File -Path (Join-Path $CaseDirectory 'expand.stderr.txt') -Contents $expansion.Stderr
    $expandedMetrics = Get-FileMetrics -Path $expandedPath

    # These patterns count only macro-generated Rust in this binary expansion:
    # helper declarations/calls, generated protocol modules, and protocol impls.
    $expanded = $expansion.Stdout
    return [pscustomobject][ordered]@{
        message_count = $MessageCount
        locale_count = $LocaleCount
        source_bytes = $SourceMetrics.Bytes
        source_lines = $SourceMetrics.Lines
        expanded_bytes = $expandedMetrics.Bytes
        expanded_lines = $expandedMetrics.Lines
        copy_helper_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*fn copy_value\b'
        copy_call_count = Get-RegexCount -Text $expanded -Pattern 'Catalog::copy_value'
        i18n_schema_module_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*pub mod __i18n_schema\b'
        i18n_catalog_module_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*pub mod __i18n_catalog\b'
        i18n_generated_module_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*pub mod __i18n_generated\b'
        catalog_impl_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*impl.*CatalogImpl\s+for\s+Catalog\b'
        static_message_impl_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*impl.*StaticMessage.*\sfor\s+'
        dynamic_message_impl_count = Get-RegexCount -Text $expanded -Pattern '(?m)^\s*impl.*DynamicMessage.*\sfor\s+'
    }
}

function Invoke-ProfileCase {
    [OutputType([pscustomobject])]
    param([pscustomobject]$ProfileCase)

    $profileRoot = [System.IO.Path]::GetFullPath((Join-Path $script:ResultsRoot 'profiles'))
    $caseDirectory = [System.IO.Path]::GetFullPath((Join-Path $profileRoot ("m{0}-l{1}" -f $ProfileCase.MessageCount, $ProfileCase.LocaleCount)))
    $profileRootPrefix = $profileRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $caseDirectory.StartsWith($profileRootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clear profile directory outside ${profileRoot}: $caseDirectory"
    }
    if (Test-Path $caseDirectory) {
        Remove-Item -LiteralPath $caseDirectory -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $caseDirectory | Out-Null
    $selfProfilePrefix = Join-Path $caseDirectory 'self-profile'
    $manifestPath = Join-Path $script:FixtureRoot 'Cargo.toml'
    Write-Fixture -MessageCount $ProfileCase.MessageCount -LocaleCount $ProfileCase.LocaleCount -Revision 0
    $sourceMetrics = Get-SourceMetrics
    $environment = Get-BenchmarkEnvironment
    [void](Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'clean', '--manifest-path', $manifestPath, '-p', 'i18n-perf-fixture') -WorkingDirectory $script:FixtureRoot -Environment $environment)

    Write-Host "PROFILE messages=$($ProfileCase.MessageCount) locales=$($ProfileCase.LocaleCount)"
    $profile = Invoke-Checked -Executable 'cargo' -Arguments @(
        '+nightly', 'rustc', '--manifest-path', $manifestPath, '--bin', 'i18n-perf-fixture', '--',
        '-Zthreads=1', '-Ztime-passes', '-Ztime-passes-format=json', '-Zmacro-stats',
        "-Zself-profile=$selfProfilePrefix", '-Zself-profile-events=default,query-keys'
    ) -WorkingDirectory $script:FixtureRoot -Environment $environment
    Write-Utf8File -Path (Join-Path $caseDirectory 'cargo-rustc.stdout.txt') -Contents $profile.Stdout
    Write-Utf8File -Path (Join-Path $caseDirectory 'cargo-rustc.stderr.txt') -Contents $profile.Stderr
    $timePasses = [System.Collections.Generic.List[string]]::new()
    $macroStats = [System.Collections.Generic.List[string]]::new()
    foreach ($line in ($profile.Stderr -split "`r?`n")) {
        if ($line -match '^time: (\{.*\})$') {
            $timePasses.Add($Matches[1])
        }
        if ($line -match '^macro-stats ') {
            $macroStats.Add($line)
        }
    }
    if ($timePasses.Count -eq 0 -or -not ($timePasses -match '"pass":"expand_crate"')) {
        throw "time-passes output for $($ProfileCase.Name) is missing expand_crate."
    }
    if ($macroStats.Count -eq 0 -or -not ($macroStats -match 'catalog!')) {
        throw "macro-stats output for $($ProfileCase.Name) is missing catalog!."
    }
    Write-Utf8File -Path (Join-Path $caseDirectory 'time-passes.jsonl') -Contents ($timePasses -join "`n")
    Write-Utf8File -Path (Join-Path $caseDirectory 'macro-stats.txt') -Contents ($macroStats -join "`n")
    $selfProfileFiles = @(Get-ChildItem -Path $selfProfilePrefix -Recurse -File -Filter '*.mm_profdata')
    if ($selfProfileFiles.Count -eq 0) {
        throw "No self-profile files were created for $($ProfileCase.Name)."
    }
    Invoke-OptionalMeasureme -CaseDirectory $caseDirectory -SelfProfileFiles $selfProfileFiles
    return Invoke-ExpansionCollection -MessageCount $ProfileCase.MessageCount -LocaleCount $ProfileCase.LocaleCount -CaseDirectory $caseDirectory -SourceMetrics $sourceMetrics
}

function Invoke-ProfileCollection {
    $rows = [System.Collections.Generic.List[object]]::new()
    foreach ($profileCase in $script:NormalizedProfileCases) {
        $rows.Add((Invoke-ProfileCase -ProfileCase $profileCase))
    }
    Write-BenchmarkCsv -Rows $rows.ToArray() -Columns @('message_count', 'locale_count', 'source_bytes', 'source_lines', 'expanded_bytes', 'expanded_lines', 'copy_helper_count', 'copy_call_count', 'i18n_schema_module_count', 'i18n_catalog_module_count', 'i18n_generated_module_count', 'catalog_impl_count', 'static_message_impl_count', 'dynamic_message_impl_count') -Path (Join-Path $script:ResultsRoot 'expansion-summary.csv')
    Write-Output "PROFILE COMPLETE cases=$($rows.Count) summary=$(Join-Path $script:ResultsRoot 'expansion-summary.csv')"
}

function Assert-SmokeFixture {
    param([string]$Source)

    $defaultEnd = $Source.IndexOf('mod locale_001')
    if ($defaultEnd -lt 0) {
        throw 'Smoke fixture is missing locale_001.'
    }
    $defaultSource = $Source.Substring(0, $defaultEnd)
    $defaultKeys = [regex]::Matches($defaultSource, '(?m)^\s*Message\d{4}\s*(?:\{|=)')
    if ($defaultKeys.Count -ne 8) {
        throw "Smoke fixture expected exactly eight default keys; found $($defaultKeys.Count)."
    }
    foreach ($declaration in @(
        'Message0000 =',
        'Message0001 {}',
        'Message0002 { first, second }',
        'Message0003 { first: usize, second: usize }',
        '..'
    )) {
        if (-not $Source.Contains($declaration)) {
            throw "Smoke fixture is missing declaration: $declaration"
        }
    }
    $revisionCount = [regex]::Matches($Source, 'BENCH_REVISION_0').Count
    if ($revisionCount -ne 1) {
        throw "Smoke fixture expected exactly one revision-zero marker; found $revisionCount."
    }
    $callSiteCount = [regex]::Matches($Source, '(?m)^\s*let _ = .*?\btr(?:_for)?!\(').Count
    if ($callSiteCount -ne 8) {
        throw "Smoke fixture expected exactly eight caller macro call sites; found $callSiteCount."
    }
}

if ($SmokeOnly) {
    Write-Fixture -MessageCount 8 -LocaleCount 2 -Revision 0
    $childEnvironment = @{
        CARGO_TARGET_DIR = $script:CargoTargetRoot
        RUSTFLAGS = '-Zthreads=1'
    }
    $lockfile = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'generate-lockfile', '--offline') -WorkingDirectory $script:FixtureRoot -Environment $childEnvironment
    $check = Invoke-Checked -Executable 'cargo' -Arguments @('+nightly', 'check', '--offline', '--locked') -WorkingDirectory $script:FixtureRoot -Environment $childEnvironment
    $source = [System.IO.File]::ReadAllText((Join-Path $script:FixtureRoot 'src\main.rs'))
    Assert-SmokeFixture -Source $source
    Write-Output "SMOKE SUCCESS messages=8 locales=2 revision=0 lock_ms=$($lockfile.ElapsedMilliseconds) check_ms=$($check.ElapsedMilliseconds)"
    return
}

Invoke-TimingMatrix
Invoke-ProfileCollection
