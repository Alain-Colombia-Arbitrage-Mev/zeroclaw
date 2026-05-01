# Loads .env.local (KEY=value lines) into the current session's env, then starts the daemon.
# Lines starting with # or empty are ignored. Quotes around values are stripped.
$envFile = Join-Path $PSScriptRoot ".env.local"
if (Test-Path $envFile) {
  Get-Content $envFile | ForEach-Object {
    $line = $_.Trim()
    if ($line -and -not $line.StartsWith("#") -and $line.Contains("=")) {
      $parts = $line -split '=', 2
      $name = $parts[0].Trim()
      $value = $parts[1].Trim().Trim('"').Trim("'")
      Set-Item -Path "Env:$name" -Value $value
    }
  }
  Write-Host "Loaded env vars from $envFile" -ForegroundColor Green
} else {
  Write-Host "No .env.local found at $envFile (continuing with current env)" -ForegroundColor Yellow
}

& "$PSScriptRoot\target\debug\zeroclaw.exe" daemon
