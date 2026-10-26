[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("rpi", "opi")]
  [string]$Board,

  [Parameter(Mandatory = $true)]
  [string]$Target
)

# Production images give the login account no passwordless sudo. The dev deploy, board timing
# smoke and Aux study need it, so a freshly flashed dev board gets one dev-only sudoers drop-in.
# sudo asks for the device password (set in the setup portal) once, on this terminal.
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "with-board-ssh.ps1")).Path

& $transport ssh -Board $Board -Target $Target "sudo -n true" 2>$null
if ($LASTEXITCODE -eq 0) {
  "Passwordless sudo already works on $Target."
  exit 0
}

$install = "printf '%s ALL=(ALL) NOPASSWD: ALL\n' `"`$(id -un)`" | sudo tee /etc/sudoers.d/octessera-dev >/dev/null" +
  " && sudo chmod 0440 /etc/sudoers.d/octessera-dev" +
  " && sudo visudo -cf /etc/sudoers.d/octessera-dev >/dev/null" +
  " || { sudo rm -f /etc/sudoers.d/octessera-dev; exit 1; }"
& $transport ssh-tty -Board $Board -Target $Target $install
if ($LASTEXITCODE -ne 0) { throw "Could not install the dev sudo rule on $Target (exit $LASTEXITCODE)." }

& $transport ssh -Board $Board -Target $Target "sudo -n true"
if ($LASTEXITCODE -ne 0) { throw "The dev sudo rule is installed but passwordless sudo still fails on $Target." }
"Dev sudo enabled on $Target (/etc/sudoers.d/octessera-dev)."
