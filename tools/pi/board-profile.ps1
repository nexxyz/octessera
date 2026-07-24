Set-StrictMode -Version Latest

$RaspberryPiZero2WProfileId = "raspberry-pi-zero-2w"
$OrangePiZero2WProfileId = "orange-pi-zero-2w"
$RaspberryPiZero2WSchemaVersion = 1
$RaspberryPiZero2WBinary = "octessera-pi"
$RaspberryPiZero2WArchitecture = "aarch64-unknown-linux-gnu"
$RaspberryPiZero2WRuntimeArchitecture = "aarch64"
$RaspberryPiZero2WCargoFeature = "hardware-raspberry-pi-zero-2w"
$RaspberryBoardMetadataFields = @(
  "schema_version",
  "board_profile",
  "binary",
  "arch",
  "cargo_feature"
)
$RaspberryRuntimeMetadataFields = @(
  "schema_version",
  "board_profile",
  "binary",
  "arch",
  "package_version"
)

function Assert-RaspberryBoardProfile {
  param([string]$BoardProfile)

  if ($BoardProfile -ceq $OrangePiZero2WProfileId) {
    throw "Orange Pi profile '$OrangePiZero2WProfileId' is not supported by Raspberry Pi tooling; use the separate Armbian workflow."
  }
  if ($BoardProfile -cne $RaspberryPiZero2WProfileId) {
    throw "Raspberry Pi tooling accepts only '$RaspberryPiZero2WProfileId'; got '$BoardProfile'."
  }
}

function Assert-OctesseraServiceName {
  param([string]$Service)

  if ($Service -cne "octessera.service") {
    throw "Pi tooling supports only the managed service name octessera.service; got '$Service'."
  }
}

function New-RaspberryBoardMetadata {
  [pscustomobject][ordered]@{
    schema_version = $RaspberryPiZero2WSchemaVersion
    board_profile = $RaspberryPiZero2WProfileId
    binary = $RaspberryPiZero2WBinary
    arch = $RaspberryPiZero2WArchitecture
    cargo_feature = $RaspberryPiZero2WCargoFeature
  }
}

function Assert-JsonObjectFields {
  param(
    [object]$Metadata,
    [string[]]$Fields,
    [string]$Context
  )

  if ($null -eq $Metadata -or $Metadata -is [array] -or $Metadata -is [string] -or $Metadata -is [ValueType]) {
    throw "$Context must be a JSON object."
  }

  $properties = @($Metadata.PSObject.Properties)
  $propertyNames = @($properties | ForEach-Object { $_.Name })
  if ($propertyNames.Count -ne $Fields.Count) {
    throw "$Context must contain exactly: $($Fields -join ', ')."
  }
  foreach ($field in $Fields) {
    if (-not ($propertyNames -ccontains $field)) {
      throw "$Context is missing required field '$field'."
    }
  }
  foreach ($propertyName in $propertyNames) {
    if (-not ($Fields -ccontains $propertyName)) {
      throw "$Context contains unexpected field '$propertyName'."
    }
  }
}

function Assert-RaspberryBoardMetadata {
  param(
    [object]$Metadata,
    [string]$Context = "Raspberry board metadata"
  )

  Assert-JsonObjectFields $Metadata $RaspberryBoardMetadataFields $Context
  if ($Metadata.schema_version -isnot [int] -and $Metadata.schema_version -isnot [long]) {
    throw "$Context schema_version must be integer $RaspberryPiZero2WSchemaVersion."
  }
  if ([long]$Metadata.schema_version -ne $RaspberryPiZero2WSchemaVersion) {
    throw "$Context schema_version must be $RaspberryPiZero2WSchemaVersion."
  }
  Assert-RaspberryBoardProfile ([string]$Metadata.board_profile)
  if ($Metadata.binary -isnot [string] -or $Metadata.binary -cne $RaspberryPiZero2WBinary) {
    throw "$Context binary must be $RaspberryPiZero2WBinary."
  }
  if ($Metadata.arch -isnot [string] -or $Metadata.arch -cne $RaspberryPiZero2WArchitecture) {
    throw "$Context arch must be $RaspberryPiZero2WArchitecture."
  }
  if ($Metadata.cargo_feature -isnot [string] -or $Metadata.cargo_feature -cne $RaspberryPiZero2WCargoFeature) {
    throw "$Context cargo_feature must be $RaspberryPiZero2WCargoFeature."
  }
  $Metadata
}

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

function Write-RaspberryBoardMetadata {
  param(
    [string]$Path,
    [string]$Binary = "octessera-pi",
    [string]$CargoFeature = "hardware-raspberry-pi-zero-2w"
  )

  $metadata = New-RaspberryBoardMetadata
  $metadata.binary = $Binary
  $metadata.cargo_feature = $CargoFeature
  Assert-RaspberryBoardMetadata ([pscustomobject]$metadata) | Out-Null
  $json = Get-RaspberryBoardMetadataJson
  $encoding = New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($Path, $json, $encoding)
}

function Get-RaspberryBoardMetadataJson {
  Assert-RaspberryBoardMetadata (New-RaspberryBoardMetadata) | Out-Null
  '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'
}

function Read-RaspberryBoardMetadata {
  param([string]$Path)

  if (-not (Test-Path -LiteralPath $Path)) {
    throw "Missing board metadata: $Path"
  }
  $json = Read-StrictUtf8Text $Path "Raspberry board metadata"
  $metadata = ConvertFrom-StrictJsonText $json "Raspberry board metadata"
  Assert-RaspberryBoardMetadata $metadata "Raspberry board metadata '$Path'"
}

function Assert-RaspberryRuntimeMetadata {
  param(
    [object]$Metadata,
    [string]$Context = "Raspberry runtime metadata"
  )

  Assert-JsonObjectFields $Metadata $RaspberryRuntimeMetadataFields $Context
  if ($Metadata.schema_version -isnot [int] -and $Metadata.schema_version -isnot [long]) {
    throw "$Context schema_version must be integer $RaspberryPiZero2WSchemaVersion."
  }
  if ([long]$Metadata.schema_version -ne $RaspberryPiZero2WSchemaVersion) {
    throw "$Context schema_version must be $RaspberryPiZero2WSchemaVersion."
  }
  Assert-RaspberryBoardProfile ([string]$Metadata.board_profile)
  if ($Metadata.binary -isnot [string] -or $Metadata.binary -cne $RaspberryPiZero2WBinary) {
    throw "$Context binary must be $RaspberryPiZero2WBinary."
  }
  if ($Metadata.arch -isnot [string] -or $Metadata.arch -cne $RaspberryPiZero2WRuntimeArchitecture) {
    throw "$Context arch must be $RaspberryPiZero2WRuntimeArchitecture."
  }
  if ($Metadata.package_version -isnot [string] -or [string]::IsNullOrWhiteSpace($Metadata.package_version)) {
    throw "$Context package_version must be a non-empty string."
  }
  $Metadata
}

function Compare-RaspberryRuntimeMetadata {
  param(
    [object]$RuntimeMetadata,
    [object]$ExpectedMetadata
  )

  Assert-RaspberryRuntimeMetadata $RuntimeMetadata | Out-Null
  Assert-RaspberryBoardMetadata $ExpectedMetadata | Out-Null
  if ([long]$RuntimeMetadata.schema_version -ne [long]$ExpectedMetadata.schema_version) {
    throw "Candidate runtime metadata schema_version does not match local metadata."
  }
  if ($RuntimeMetadata.board_profile -cne $ExpectedMetadata.board_profile) {
    throw "Candidate runtime metadata board_profile does not match local metadata."
  }
  if ($RuntimeMetadata.binary -cne $ExpectedMetadata.binary) {
    throw "Candidate runtime metadata binary does not match local metadata."
  }
  $expectedArchitecture = ([string]$ExpectedMetadata.arch).Split("-", 2)[0]
  if ($RuntimeMetadata.arch -cne $expectedArchitecture) {
    throw "Candidate runtime metadata arch does not match local metadata."
  }
  $RuntimeMetadata
}
