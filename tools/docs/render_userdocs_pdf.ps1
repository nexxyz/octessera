param(
  [string]$Chrome = "$env:ProgramFiles\Google\Chrome\Application\chrome.exe"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Chrome)) {
  throw "Chrome not found at $Chrome. Pass -Chrome with a Chromium/Chrome executable."
}

$root = Resolve-Path "."

if (-not (Test-Path -LiteralPath "userdocs/print/cell-to-audio-flow.svg")) {
  throw "Missing userdocs/print/cell-to-audio-flow.svg"
}

$jobs = @(
  @{ Source = "userdocs/print/quick-reference.html"; Output = "userdocs/print/quick-reference.pdf" }
)

foreach ($job in $jobs) {
  $source = (Resolve-Path $job.Source).Path
  $output = Join-Path $root $job.Output
  $url = ([System.Uri]::new($source)).AbsoluteUri
  & $Chrome `
    --headless `
    --disable-gpu `
    --no-pdf-header-footer `
    --print-to-pdf="$output" `
    $url
  for ($attempt = 0; $attempt -lt 20 -and -not (Test-Path -LiteralPath $output); $attempt++) {
    Start-Sleep -Milliseconds 250
  }
  if (-not (Test-Path -LiteralPath $output)) {
    throw "Chrome did not write $output; exit code $LASTEXITCODE"
  }
  "wrote $output"
}
