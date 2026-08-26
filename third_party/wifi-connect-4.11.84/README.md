# Octessera-patched wifi-connect 4.11.84

This directory keeps the small Octessera source patch for the pinned
wifi-connect release. The upstream source is
`balena-os/wifi-connect` v4.11.84 at commit
`5bd4c1bea548fb5714bedb18bbd12f088d5fa407`. The read-only inspection clone is
`.slim/clonedeps/repos/balena-os__wifi-connect`; it must never be edited.

## Why the patch exists

On the fixed boards, NetworkManager can report the hotspot connection before
the configured gateway address is usable in the service namespace. The pinned
wifi-connect process then races dnsmasq and its HTTP server, producing the live
`EADDRNOTAVAIL`/unknown-`wlan0` early-start failure. At the pinned
`network-manager` commit, `create_hotspot()` returns the connection and its
initial state, while upstream discards that state and logs the portal as
created. The patch preserves it, calls `Connection::activate()` exactly once
when the initial state is not `Activated`, and accepts only a final
`Activated` state. Other final states return `PortalActivation` with stable
exit code 26; activation API errors propagate unchanged.

Only after activation succeeds does the patch wait on the configured
`SocketAddrV4` before starting dnsmasq and HTTP. It retries only
`AddrNotAvailable`, polls every 100 ms for at most 10 seconds, and fails
immediately for address-in-use, permission, or other errors.

The patch changes only `src/network.rs` and `src/errors.rs`. It adds focused
source tests for activation-call cardinality, non-activated final states,
activation API error propagation, transient address recovery, truthful
timeout reporting, and immediate non-transient failure. It does not inspect
the Internet, call a shell, change NetworkManager, or alter the captive-portal
UI.

## Reproduce and verify

From the repository root, use:

```powershell
pwsh -File tools/wifi-connect/build-patched.ps1
```

The builder verifies the clone origin and exact commit, copies it to
`target/wifi-connect-patched/source`, applies
`portal-address-readiness.patch` with `git apply --check`, runs locked native
tests, builds the locked AArch64 release in the pinned `rust:1.76.0-bookworm`
image (`sha256:d36f9d8a9a4c76da74c8d983d0d4cb146dd2d19bb9bd60b704cdcf70ef868d3a`),
strips and checks the ELF, and writes:

- `target/wifi-connect-patched/wifi-connect`
- `target/wifi-connect-patched/wifi-connect.metadata.json`
- `target/wifi-connect-patched/cargo-metadata.json`

Run the source-only checks without building with:

```powershell
python tools/wifi-connect/test-patched-source.py
```

The build metadata records the upstream ref and commit, patch SHA256, target,
container/toolchain, and final binary SHA256. The builder refuses to publish a
binary unless it matches the pinned binary and patch hashes. Image constructors
consume the locally staged output and retain the binary, metadata, Cargo
metadata, license, and third-party notice under the image's local documentation
tree.

## License and dependency inventory

wifi-connect is distributed under Apache-2.0; the exact upstream license is
included here. Modified source carries the required Octessera notice. The
locked `network-manager` dependency is pinned to commit
`4da2e6a57de16b6ae911f74321f929d78af8b1ba` and is Apache-2.0. The builder
emits `cargo metadata --locked --format-version 1` as the input to the local
transitive dependency inventory. This lane does not claim that every
transitive license has been independently verified; that remains a separate
licensing gate.
