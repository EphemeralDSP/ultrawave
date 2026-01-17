# VST3 Plugin Validation Script using pluginval
# This script automates VST3 plugin validation with configurable strictness levels
#
# Usage:
#   .\validate-vst3.ps1 [-PluginPath <path>] [-Strictness <1-10>] [-Verbose]
#
# References:
#   - https://github.com/Tracktion/pluginval
#   - https://github.com/Tracktion/pluginval/blob/develop/docs/Testing%20plugins%20with%20pluginval.md

param(
    [Parameter(Mandatory=$false)]
    [string]$PluginPath = "",

    [Parameter(Mandatory=$false)]
    [ValidateRange(1,10)]
    [int]$Strictness = 5,

    [Parameter(Mandatory=$false)]
    [switch]$Verbose,

    [Parameter(Mandatory=$false)]
    [string]$PluginvalPath = ""
)

$ErrorActionPreference = "Stop"

# ANSI color codes for output
$RED = "`e[31m"
$GREEN = "`e[32m"
$YELLOW = "`e[33m"
$BLUE = "`e[34m"
$RESET = "`e[0m"

function Write-Info($message) {
    Write-Host "${BLUE}[INFO]${RESET} $message"
}

function Write-Success($message) {
    Write-Host "${GREEN}[SUCCESS]${RESET} $message"
}

function Write-Warning($message) {
    Write-Host "${YELLOW}[WARNING]${RESET} $message"
}

function Write-Error($message) {
    Write-Host "${RED}[ERROR]${RESET} $message"
}

# Find pluginval executable
function Find-Pluginval {
    if ($PluginvalPath -and (Test-Path $PluginvalPath)) {
        return $PluginvalPath
    }

    # Check common locations
    $possiblePaths = @(
        ".\tools\pluginval.exe",
        "$env:USERPROFILE\pluginval\pluginval.exe",
        "C:\Program Files\pluginval\pluginval.exe",
        "$env:LOCALAPPDATA\pluginval\pluginval.exe"
    )

    foreach ($path in $possiblePaths) {
        if (Test-Path $path) {
            Write-Info "Found pluginval at: $path"
            return $path
        }
    }

    # Try to find in PATH
    $pluginvalCmd = Get-Command pluginval -ErrorAction SilentlyContinue
    if ($pluginvalCmd) {
        Write-Info "Found pluginval in PATH: $($pluginvalCmd.Source)"
        return $pluginvalCmd.Source
    }

    Write-Error "pluginval not found. Please install it from https://github.com/Tracktion/pluginval"
    Write-Info "Installation options:"
    Write-Info "  1. Download from: https://github.com/Tracktion/pluginval/releases"
    Write-Info "  2. Place in .\tools\pluginval.exe"
    Write-Info "  3. Or specify path with -PluginvalPath parameter"
    exit 1
}

# Find VST3 plugin to test
function Find-Plugin {
    param([string]$path)

    if ($path -and (Test-Path $path)) {
        return $path
    }

    # Look for built VST3 in target directory
    $buildPaths = @(
        ".\target\debug\Ultrawave.vst3",
        ".\target\release\Ultrawave.vst3",
        ".\target\bundled\Ultrawave.vst3"
    )

    foreach ($buildPath in $buildPaths) {
        if (Test-Path $buildPath) {
            Write-Info "Found VST3 plugin at: $buildPath"
            return $buildPath
        }
    }

    Write-Error "No VST3 plugin found. Please build the plugin first or specify path with -PluginPath"
    exit 1
}

# Main validation logic
function Invoke-Validation {
    Write-Info "========================================="
    Write-Info "VST3 Plugin Validation"
    Write-Info "========================================="
    Write-Info ""

    $pluginval = Find-Pluginval
    $plugin = Find-Plugin -path $PluginPath

    Write-Info "Pluginval: $pluginval"
    Write-Info "Plugin: $plugin"
    Write-Info "Strictness Level: $Strictness (1=lenient, 10=strict)"
    Write-Info ""

    # Build pluginval arguments
    $args = @(
        "--validate-in-process",
        "--strictness-level", $Strictness,
        "--verbose"
    )

    if (-not $Verbose) {
        $args = $args | Where-Object { $_ -ne "--verbose" }
    }

    $args += $plugin

    Write-Info "Running validation..."
    Write-Info "Command: $pluginval $($args -join ' ')"
    Write-Info ""

    # Run pluginval
    $startTime = Get-Date
    try {
        & $pluginval @args
        $exitCode = $LASTEXITCODE
    } catch {
        Write-Error "Failed to run pluginval: $_"
        exit 1
    }
    $endTime = Get-Date
    $duration = $endTime - $startTime

    Write-Info ""
    Write-Info "========================================="
    Write-Info "Validation Results"
    Write-Info "========================================="
    Write-Info "Duration: $($duration.TotalSeconds) seconds"
    Write-Info ""

    if ($exitCode -eq 0) {
        Write-Success "✓ All tests passed!"
        Write-Success "Plugin is compatible with strictness level $Strictness"
        Write-Info ""
        Write-Info "Strictness level meanings:"
        Write-Info "  5+ : Recommended for broad DAW compatibility"
        Write-Info "  8+ : Suitable for 'Verified by pluginval' badge"
        Write-Info "  10 : Maximum strictness (all tests)"
        return 0
    } else {
        Write-Error "✗ Validation failed (exit code: $exitCode)"
        Write-Warning "Some tests did not pass at strictness level $Strictness"
        Write-Info ""
        Write-Info "Next steps:"
        Write-Info "  1. Review the output above for specific failures"
        Write-Info "  2. Fix the reported issues in your plugin"
        Write-Info "  3. Run validation again"
        Write-Info "  4. Consider reducing strictness temporarily if needed"
        return $exitCode
    }
}

# Run validation
$result = Invoke-Validation
exit $result
