# Octessera setup portal

Static UI for the one-page branded dark Wi-Fi setup portal.

## Page fields

- Country code
- A network from the scanned list
- Its Wi-Fi password (none for open networks)
- Local device password and confirmation; it is required even when SSH is disabled
- SSH key, SSH password, or none
- Optional hostname
- Apply

## Backend calls

`setup_http.py` serves this page and every call on `http://192.168.42.1/`;
the page uses relative paths only. Any other host name gets a redirect to
`http://192.168.42.1/`.

- `GET /networks` returns a JSON array of `{ssid, security, strength}` with
  security `none` or `wpa` and strength 0-100, strongest first and one entry
  per SSID, from the one scan before the hotspot started; the page keeps that
  order. WEP and enterprise networks are left out.
- `POST /country` applies the two-letter country code to the running radio.
- `POST /stage` validates the country, Wi-Fi network and password, local device
  password, SSH, and hostname fields and passes them to the root coordinator in
  memory. The network must be one from `/networks`.

## `/stage` request body

Send JSON to `/stage` with these exact fields:

```json
{
  "sshMode": "none",
  "sshPublicKey": "",
  "accountPassword": "Example-only-123!",
  "accountPasswordConfirm": "Example-only-123!",
  "hostname": "",
  "wifiCountry": "US",
  "wifiSsid": "Home",
  "wifiPassphrase": "Example-wifi-1"
}
```

The example password is fictional; do not reuse it.

`sshMode` must be one of `none`, `key`, or `password`. `wifiPassphrase` is
empty for an open network and 8-63 printable ASCII characters (or 64 hex
digits) otherwise.

The local device password is set for the board account even when `sshMode` is
`none`. Key mode installs the public key and keeps SSH password login disabled.
Password mode enables SSH password login using the same local device password.
None mode keeps SSH disabled while leaving the local console and attended sudo
password available.

## `/country` request body

Send JSON to `/country` with exactly one field:

```json
{"wifiCountry":"US"}
```

The endpoint accepts two ASCII letters and applies the country to the running
radio. It does not write credentials.

## Behavior

- Apply `/country` before showing the scanned list and before every refresh.
- Show the provisional `Applying setup` screen after `/stage` succeeds. About a
  second later the hotspot goes away while the instrument joins the chosen
  network; watch the OLED for the result and find the address in
  `System > Maintenance > Sys. Info` after success.
- The browser makes no completion or retry call. Keep form state in memory only.
- Do not rely on external services or storage.
