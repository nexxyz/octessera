# Octessera setup portal

Static UI for the first-boot wifi-connect captive portal.

## Backend calls

- `GET /networks`
- `POST /stage`
- `POST /connect`

## `/stage` request body

Send JSON to `http://192.168.42.1:8080/stage` with these exact fields:

```json
{
  "sshMode": "none",
  "sshPublicKey": "",
  "sshPassword": "",
  "sshPasswordConfirm": "",
  "hostname": "",
  "wifiCountry": "US"
}
```

`sshMode` must be one of `none`, `key`, or `password`.

## `/connect` request body

Send form-urlencoded fields:

- `ssid`
- `identity`
- `passphrase`

`identity` may be blank.

## Behavior

- Call `/stage` before `/connect`.
- Keep all form state in memory only.
- Do not rely on external assets, fonts, or storage.
