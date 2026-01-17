param($Help)
if ($Help) { Write-Host "Help"; exit 0 }
function TestFunc { Write-Host "Test" }
TestFunc
