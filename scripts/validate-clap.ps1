# CLAP Plugin Validation Script using clap-validator
# This script automates CLAP plugin validation with configurable test selection
#
# Usage:
#   .\validate-clap.ps1 [-PluginPath <path>] [-Tests <test-list>] [-Detailed]
#
# References:
#   - https://github.com/free-audio/clap-validator
#   - https://cleveraudio.org/developers-getting-started/

param(
    [string]$PluginPath = "",
    [string]$Tests = "",
    [switch]$Detailed,
    [string]$ValidatorPath = "",
    [switch]$ListTests
)

$ErrorActionPreference = "Stop"

# ANSI color codes for output (safer version)
$ESC = [char]27
$RED = "$ESC[31m"
$GREEN = "$ESC[32m"
$YELLOW = "$ESC[33m"
$BLUE = "$ESC[34m"
$RESET = "$ESC[0m"

function Write-Info($message) {
    Write-Host ("${BLUE}[INFO]${RESET} " + $message)
}

function Write-Success($message) {
    Write-Host ("${GREEN}[SUCCESS]${RESET} " + $message)
}

function Write-Warning($message) {
    Write-Host ("${YELLOW}[WARNING]${RESET} " + $message)
}

function Write-Error($message) {
    Write-Host ("${RED}[ERROR]${RESET} " + $message)
}

# Find clap-validator executable
function Find-Validator {
    if ($ValidatorPath -and (Test-Path $ValidatorPath)) {
        return $ValidatorPath
    }

    # Try to find in PATH
    $validatorCmd = Get-Command clap-validator -ErrorAction SilentlyContinue
    if ($validatorCmd) {
        Write-Info ("Found clap-validator in PATH: " + $validatorCmd.Source)
        return $validatorCmd.Source
    }

    # Check common cargo install location
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin\clap-validator.exe"
    if (Test-Path $cargoBin) {
        Write-Info ("Found clap-validator in cargo bin: " + $cargoBin)
        return $cargoBin
    }

    Write-Error "clap-validator not found. Please install it from https://github.com/free-audio/clap-validator"
    Write-Info "Installation options:"
    Write-Info "  1. Build from source: cargo install --git https://github.com/free-audio/clap-validator"
    Write-Info "  2. Download from: https://github.com/free-audio/clap-validator/releases"
    Write-Info "  3. Or specify path with -ValidatorPath parameter"
    exit 1
}

# Find CLAP plugin to test
function Find-Plugin {
    param([string]$path)

    if ($path -and (Test-Path $path)) {
        return $path
    }

    # Look for built CLAP in target directory
    $buildPaths = @(
        ".\target\debug\Ultrawave.clap",
        ".\target\release\Ultrawave.clap",
        ".\target\bundled\Ultrawave.clap"
    )

    foreach ($buildPath in $buildPaths) {
        if (Test-Path $buildPath) {
            Write-Info ("Found CLAP plugin at: " + $buildPath)
            return $buildPath
        }
    }

    Write-Error "No CLAP plugin found. Please build the plugin first or specify path with -PluginPath"
    exit 1
}

# Main validation logic
function Invoke-Validation {
    if ($ListTests) {
        $v = Find-Validator
        & $v list tests
        exit 0
    }

    Write-Info "========================================="
    Write-Info "CLAP Plugin Validation"
    Write-Info "========================================="
    Write-Info ""

    $v = Find-Validator
    $p = Find-Plugin -path $PluginPath

    Write-Info ("Validator: " + $v)
    Write-Info ("Plugin: " + $p)
    if ($Tests) {
        Write-Info ("Tests: " + $Tests)
    } else {
        Write-Info "Tests: All tests (default)"
    }
    Write-Info ""

    # Build validator arguments
    $valArgs = @("validate")

    if ($Detailed) {
        $valArgs += "--verbose"
    }

    if ($Tests) {
        $valArgs += "--only"
        $valArgs += $Tests
    }

    $valArgs += $p

    Write-Info "Running validation..."
    Write-Info ("Command: " + $v + " " + ($valArgs -join ' '))
    Write-Info ""

    # Run clap-validator
    $startTime = Get-Date
    try {
        & $v $valArgs
        $exitCode = $LASTEXITCODE
    } catch {
        Write-Error ("Failed to run clap-validator: " + $_)
        exit 1
    }
    $endTime = Get-Date
    $duration = $endTime - $startTime

    Write-Info ""
    Write-Info "========================================="
    Write-Info "Validation Results"
    Write-Info "========================================="
    Write-Info ("Duration: " + $duration.TotalSeconds + " seconds")
    Write-Info ""

    if ($exitCode -eq 0) {
        Write-Success "PASSED: All tests passed!"
        Write-Success "Plugin passed CLAP validation"
        Write-Info ""
        Write-Success "Test categories validated:"
        Write-Success "  - Fuzzing tests"
        Write-Success "  - Thread safety checks"
        Write-Success "  - State validation"
        Write-Success "  - Parameter validation"
        if (-not $Tests) {
            Write-Success "  - All plugin tests"
        }
        return 0
    } else {
        Write-Error ("FAILED: Validation failed (exit code: " + $exitCode + ")")
        Write-Warning "Some tests did not pass"
        Write-Info ""
        Write-Info "Next steps:"
        Write-Info "  1. Review the output above for specific failures"
        Write-Info "  2. Fix the reported issues in your plugin"
        Write-Info "  3. Run validation again"
        Write-Info "  4. Use -Tests to run specific failing tests"
        Write-Info "  5. Use -ListTests to see all available tests"
        return $exitCode
    }
}

# Run validation
$res = Invoke-Validation
exit $res
