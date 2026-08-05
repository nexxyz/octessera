Set-StrictMode -Version Latest

function Assert-PiDeploymentTarget {
  param([AllowNull()][AllowEmptyString()][string]$Target)

  if ([string]::IsNullOrEmpty($Target)) {
    throw "Pi deployment target must be a non-empty user@IPv4 or user@hostname value."
  }

  foreach ($character in $Target.ToCharArray()) {
    if ([char]::IsControl($character) -or [char]::IsWhiteSpace($character)) {
      throw "Pi deployment target must not contain control characters or whitespace."
    }
  }
  if ($Target[0] -eq '-') {
    throw "Pi deployment target must not begin with a host option."
  }

  $atIndex = $Target.IndexOf('@')
  if ($atIndex -le 0 -or $atIndex -ne $Target.LastIndexOf('@')) {
    throw "Pi deployment target must contain exactly one user@host separator."
  }

  $username = $Target.Substring(0, $atIndex)
  $hostname = $Target.Substring($atIndex + 1)
  if ($username -notmatch '^[A-Za-z_][A-Za-z0-9_.-]*$') {
    throw "Pi deployment target contains an unsafe username."
  }
  if ([string]::IsNullOrEmpty($hostname) -or $hostname[0] -eq '-') {
    throw "Pi deployment target contains an invalid host."
  }
  if ($hostname.Length -gt 253) {
    throw "Pi deployment target host is too long."
  }

  if ($hostname -match '^[0-9.]+$') {
    if ($hostname -notmatch '^\d+\.\d+\.\d+\.\d+$') {
      throw "Pi deployment target contains a malformed IPv4 address."
    }
    foreach ($octet in $hostname.Split('.')) {
      if ($octet.Length -gt 1 -and $octet[0] -eq '0') {
        throw "Pi deployment target contains a malformed IPv4 address."
      }
      $value = 0
      if (-not [int]::TryParse($octet, [Globalization.NumberStyles]::None, [Globalization.CultureInfo]::InvariantCulture, [ref]$value) -or $value -gt 255) {
        throw "Pi deployment target contains a malformed IPv4 address."
      }
    }
    return $Target
  }

  $label = '[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?'
  if ($hostname -notmatch "^(?:$label)(?:\.(?:$label))*$") {
    throw "Pi deployment target contains a malformed hostname."
  }
  $Target
}
