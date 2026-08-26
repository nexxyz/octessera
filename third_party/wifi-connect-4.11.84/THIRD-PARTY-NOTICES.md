# Third-party notice: wifi-connect

- Project: balena-os/wifi-connect, version 4.11.84
- Upstream commit: `5bd4c1bea548fb5714bedb18bbd12f088d5fa407`
- License: Apache-2.0; see [`LICENSE`](LICENSE)
- Modified files: `src/network.rs` and `src/errors.rs`
- Modification purpose: require the portal connection to reach
  `ConnectionState::Activated` before waiting for the configured portal IPv4
  address and listening port, then start dnsmasq and HTTP only after bounded
  transient `AddrNotAvailable` retry. Non-activated final states use the
  truthful `PortalActivation` error with stable exit code 26.

This project-owned patch is not endorsed by balena or the wifi-connect
maintainers. The upstream project and its marks remain their respective
owners' property.

The locked `network-manager` dependency is pinned to commit
`4da2e6a57de16b6ae911f74321f929d78af8b1ba` under Apache-2.0. The reproducible
builder emits locked Cargo metadata as the input to the transitive license
inventory gate. That metadata is an inventory input; this lane does not claim
that all transitive licenses have been independently verified.
