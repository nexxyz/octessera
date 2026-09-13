# Octessera setup portal

Static UI for the one-page branded dark Wi-Fi setup portal.

## Page fields

- Country code
- Scanned or manual SSID
- Wi-Fi password or open network
- Local device password and confirmation; it is required even when SSH is disabled
- SSH key, SSH password, or none
- Optional hostname
- Apply

## Backend calls

- `GET /networks` and `POST /connect` are served by the pinned patched
  wifi-connect. It owns the setup AP, DHCP, HTTP portal, and network switch.
- `POST http://192.168.42.1:8080/country` applies the two-letter country code to
  the running radio.
- `POST http://192.168.42.1:8080/stage` validates the country, local device
  password, SSH, and hostname fields and passes them to the root coordinator in
  memory.

## `/stage` request body

Send JSON to `http://192.168.42.1:8080/stage` with these exact fields:

```json
{
  "sshMode": "none",
  "sshPublicKey": "",
  "accountPassword": "Example-only-123!",
  "accountPasswordConfirm": "Example-only-123!",
  "hostname": "",
  "wifiCountry": "US"
}
```

The example password is fictional; do not reuse it.

`sshMode` must be one of `none`, `key`, or `password`.

The local device password is set for the board account even when `sshMode` is
`none`. Key mode installs the public key and keeps SSH password login disabled.
Password mode enables SSH password login using the same local device password.
None mode keeps SSH disabled while leaving the local console and attended sudo
password available.

## `/country` request body

Send JSON to `http://192.168.42.1:8080/country` with exactly one field:

```json
{"wifiCountry":"US"}
```

The endpoint accepts two ASCII letters and applies the country to the running
radio. It does not write credentials.

## `/connect` request body

Send form-urlencoded fields:

- `ssid`
- `identity`
- `passphrase`

`identity` may be blank.

## Behavior

- Apply `/country` before the initial network scan and before every refresh.
- Call `/stage` before `/connect`.
- Show the provisional `Applying setup` screen after `/stage` succeeds and
  before `/connect`. A response only means that wifi-connect accepted the
  request. The AP may disconnect while settings apply; watch the OLED for the
  result and find the address in `System > Sys. Info` after success.
- The browser makes no completion or retry call. Keep form state in memory only.
- Do not rely on external services or storage.
