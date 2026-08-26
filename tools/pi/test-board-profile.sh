#!/usr/bin/env bash
set -euo pipefail

source ./tools/pi/board-profile.sh

[[ "$(get_pi_board_cargo_feature "$RASPBERRY_PI_ZERO_2W_PROFILE_ID")" == "$RASPBERRY_PI_ZERO_2W_CARGO_FEATURE" ]]
[[ "$(get_pi_board_cargo_feature "$ORANGE_PI_ZERO_2W_PROFILE_ID")" == "$ORANGE_PI_ZERO_2W_CARGO_FEATURE" ]]

for profile in "opi-zero-2w" "rpi-zero-2w" "hardware-pi"; do
  if require_supported_pi_board_profile "$profile"; then
    echo "Pi cross-build accepted non-canonical profile: $profile" >&2
    exit 1
  fi
done

if require_raspberry_pi_board_profile "$ORANGE_PI_ZERO_2W_PROFILE_ID"; then
  echo "Raspberry Pi tooling accepted the Orange profile" >&2
  exit 1
fi

cross_build_script="$(cat ./tools/pi/build-pi-cross.ps1)"
wsl_cross_build_script="$(cat ./tools/pi/build-pi-cross-wsl.sh)"
if ! grep -Eq '\$dockerfilePath[[:space:]]*=[[:space:]]*Join-Path \$buildContext "Dockerfile\.pi-zero"' <<<"$cross_build_script"; then
  echo "Pi PowerShell cross-build must place its Dockerfile in the temporary build context" >&2
  exit 1
fi
if ! grep -Eq 'Copy-Item -LiteralPath \(Join-Path \$RepoRoot "Dockerfile\.pi-zero"\) -Destination \$dockerfilePath' <<<"$cross_build_script"; then
  echo "Pi PowerShell cross-build must copy the repository Dockerfile into the temporary build context" >&2
  exit 1
fi
if ! grep -Eq 'docker build[[:space:]]+-f[[:space:]]+\$dockerfilePath[[:space:]]+-t[[:space:]]+\$Image[[:space:]]+\$buildContext' <<<"$cross_build_script"; then
  echo "Pi PowerShell cross-build must use the temporary context and Dockerfile" >&2
  exit 1
fi
if grep -Eq 'docker build[[:space:]]+.*[[:space:]]\.[[:space:]]*$' <<<"$cross_build_script"; then
  echo "Pi PowerShell cross-build must not use the repository root as Docker build context" >&2
  exit 1
fi
if ! grep -Eq 'DOCKERFILE="\$BUILD_CONTEXT/Dockerfile\.pi-zero"' <<<"$wsl_cross_build_script"; then
  echo "Pi WSL cross-build must place its Dockerfile in the temporary build context" >&2
  exit 1
fi
if ! grep -Eq 'cp "\$PWD/Dockerfile\.pi-zero" "\$DOCKERFILE"' <<<"$wsl_cross_build_script"; then
  echo "Pi WSL cross-build must copy the repository Dockerfile into the temporary build context" >&2
  exit 1
fi
if ! grep -Eq 'docker build[[:space:]]+-f[[:space:]]+"\$DOCKERFILE"[[:space:]]+-t[[:space:]]+"\$IMAGE"[[:space:]]+"\$BUILD_CONTEXT"' <<<"$wsl_cross_build_script"; then
  echo "Pi WSL cross-build must use the temporary context and Dockerfile" >&2
  exit 1
fi
if grep -Eq 'docker build[[:space:]]+.*[[:space:]]\.[[:space:]]*$' <<<"$wsl_cross_build_script"; then
  echo "Pi WSL cross-build must not use the repository root as Docker build context" >&2
  exit 1
fi

echo "Shell board profile validation passed"
