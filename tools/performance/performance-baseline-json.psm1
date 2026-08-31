Set-StrictMode -Version Latest

function Get-JsonStringEnd {
  param(
    [string]$Text,
    [int]$Start
  )

  for ($index = $Start + 1; $index -lt $Text.Length; $index++) {
    if ($Text[$index] -eq "\") {
      $index++
      continue
    }
    if ($Text[$index] -eq '"') {
      return $index
    }
  }
  throw "JSON contains an unterminated string."
}

function ConvertFrom-JsonStringToken {
  param(
    [string]$Text,
    [int]$Start,
    [int]$End
  )

  $builder = New-Object System.Text.StringBuilder
  for ($index = $Start + 1; $index -lt $End; $index++) {
    if ($Text[$index] -ne "\") {
      [void]$builder.Append($Text[$index])
      continue
    }
    $index++
    if ($index -ge $End) {
      throw "JSON contains an incomplete string escape."
    }
    switch ($Text[$index]) {
      '"' { [void]$builder.Append('"') }
      '\' { [void]$builder.Append("\") }
      '/' { [void]$builder.Append('/') }
      'b' { [void]$builder.Append([char]8) }
      'f' { [void]$builder.Append([char]12) }
      'n' { [void]$builder.Append("`n") }
      'r' { [void]$builder.Append("`r") }
      't' { [void]$builder.Append("`t") }
      'u' {
        if ($index + 4 -ge $End) {
          throw "JSON contains an incomplete unicode escape."
        }
        $hex = $Text.Substring($index + 1, 4)
        if ($hex -notmatch '^[0-9A-Fa-f]{4}$') {
          throw "JSON contains an invalid unicode escape."
        }
        [void]$builder.Append([char][Convert]::ToInt32($hex, 16))
        $index += 4
      }
      default { throw "JSON contains an invalid string escape." }
    }
  }
  $builder.ToString()
}

function Skip-JsonWhitespace {
  param(
    [string]$Text,
    [int]$Start
  )

  $index = $Start
  while ($index -lt $Text.Length -and [char]::IsWhiteSpace($Text[$index])) {
    $index++
  }
  $index
}

function Skip-JsonValue {
  param(
    [string]$Text,
    [int]$Start
  )

  if ($Start -ge $Text.Length) {
    throw "JSON is missing a value."
  }
  if ($Text[$Start] -eq '"') {
    return (Get-JsonStringEnd $Text $Start) + 1
  }
  if ($Text[$Start] -eq '{' -or $Text[$Start] -eq '[') {
    $depth = 0
    $inString = $false
    for ($index = $Start; $index -lt $Text.Length; $index++) {
      if ($inString) {
        if ($Text[$index] -eq "\") {
          $index++
        } elseif ($Text[$index] -eq '"') {
          $inString = $false
        }
        continue
      }
      if ($Text[$index] -eq '"') {
        $inString = $true
      } elseif ($Text[$index] -eq '{' -or $Text[$index] -eq '[') {
        $depth++
      } elseif ($Text[$index] -eq '}' -or $Text[$index] -eq ']') {
        $depth--
        if ($depth -eq 0) {
          return $index + 1
        }
      }
    }
    throw "JSON contains an unterminated object or array."
  }
  $index = $Start
  while ($index -lt $Text.Length -and $Text[$index] -notmatch '[\s,}\]]') {
    $index++
  }
  if ($index -eq $Start) {
    throw "JSON is missing a value."
  }
  $index
}

function Assert-UniqueJsonObjectFields {
  param(
    [string]$Text,
    [string]$Context
  )

  $index = Skip-JsonWhitespace $Text 0
  if ($index -ge $Text.Length -or $Text[$index] -ne '{') {
    return
  }
  $index++
  $first = $true
  $names = New-Object 'System.Collections.Generic.Dictionary[string,bool]' ([StringComparer]::Ordinal)
  while ($true) {
    $index = Skip-JsonWhitespace $Text $index
    if ($index -ge $Text.Length) {
      throw "$Context is missing its closing object brace."
    }
    if ($Text[$index] -eq '}' -and $first) {
      return
    }
    if (-not $first) {
      if ($Text[$index] -ne ',') {
        throw "$Context is missing a comma between fields."
      }
      $index = Skip-JsonWhitespace $Text ($index + 1)
    }
    if ($index -ge $Text.Length -or $Text[$index] -ne '"') {
      throw "$Context has an invalid field name."
    }
    $end = Get-JsonStringEnd $Text $index
    $name = ConvertFrom-JsonStringToken $Text $index $end
    if ($names.ContainsKey($name)) {
      throw "$Context contains duplicate field '$name'."
    }
    $names[$name] = $true
    $index = Skip-JsonWhitespace $Text ($end + 1)
    if ($index -ge $Text.Length -or $Text[$index] -ne ':') {
      throw "$Context field '$name' is missing a colon."
    }
    $index = Skip-JsonValue $Text (Skip-JsonWhitespace $Text ($index + 1))
    $first = $false
    $index = Skip-JsonWhitespace $Text $index
    if ($index -lt $Text.Length -and $Text[$index] -eq '}') {
      return
    }
    if ($index -ge $Text.Length -or $Text[$index] -ne ',') {
      throw "$Context is missing its closing object brace."
    }
  }
}

function ConvertFrom-StrictJsonText {
  param(
    [string]$Text,
    [string]$Context = "JSON"
  )

  if ([string]::IsNullOrWhiteSpace($Text)) {
    throw "$Context is empty."
  }
  if ($Text.Length -gt 0 -and [int][char]$Text[0] -eq 0xFEFF) {
    throw "$Context must not contain a UTF-8 BOM."
  }
  Assert-UniqueJsonObjectFields $Text $Context
  try {
    $Metadata = $Text | ConvertFrom-Json -ErrorAction Stop
  } catch {
    throw "$Context is malformed JSON: $($_.Exception.Message)"
  }
  if ($null -eq $Metadata) {
    throw "$Context must be a JSON object."
  }
  $Metadata
}

function Read-StrictUtf8Text {
  param(
    [string]$Path,
    [string]$Context = "JSON"
  )

  try {
    $bytes = [IO.File]::ReadAllBytes($Path)
  } catch {
    throw "Unable to read $Context '$Path': $($_.Exception.Message)"
  }
  if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
    throw "$Context must be BOM-less UTF-8: $Path"
  }
  $encoding = New-Object System.Text.UTF8Encoding($false, $true)
  try {
    $encoding.GetString($bytes)
  } catch {
    throw "$Context must be valid UTF-8: $Path"
  }
}

Export-ModuleMember -Function @(
  "Assert-UniqueJsonObjectFields",
  "ConvertFrom-StrictJsonText",
  "Read-StrictUtf8Text"
)
