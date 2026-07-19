# Orange Pi first boot setup

The Orange Pi image starts a small setup website if it does not already know a Wi-Fi network.

Use this before final assembly if you want. You do not need the OLED or buttons installed yet.

## First boot

1. Flash the Octessera Orange Pi Armbian image to a microSD card.
2. Put the card in the Orange Pi and power it on.
3. Wait for a Wi-Fi network named `Octessera Setup` or `Octessera Setup xxxx`.
4. Join that network from a phone or laptop.
5. Open the setup page if it does not appear automatically:

   ```text
   http://192.168.42.1/
   ```

6. Choose your Wi-Fi network.
7. Pick SSH access:
   - SSH key is best. The key becomes the admin credential and can use `sudo` without a password.
   - SSH password works if you need it. The same password is used for SSH login and `sudo`.
   - You can also leave SSH off.
8. Set a hostname if you want one.
9. Press the final connect button.

The setup hotspot disappears when the Orange Pi joins your Wi-Fi. That is the good kind of vanishing trick.

## Security note

The setup hotspot is for nearby, first-boot setup. Until setup finishes, anyone close enough to join that hotspot can configure the device.

Set it up near the device. Do not leave it powered on in setup mode in a public place. SSH keys are safer than passwords.

Octessera does not add its own shared SSH password or baked SSH key. The underlying Armbian image may still expose its normal first-run console/bootstrap credentials. If you use that path instead of the setup portal, change the default password immediately.

The setup portal creates or updates Octessera's SSH access. It does not scrub Armbian's own root/bootstrap credentials from the image, though Octessera still keeps network SSH closed until setup enables it.

## If setup does not appear

- Give the Orange Pi a minute or two after first power-on.
- If the setup hotspot disappeared before you finished, reboot the Orange Pi or restart `octessera-setup.service` from console. The setup hotspot intentionally times out instead of staying open forever.
- Check that your phone or laptop is not clinging to another Wi-Fi network.
- Try opening `http://192.168.42.1/` directly.
- If the setup network never appears, use serial/console access and check:

  ```sh
  systemctl status octessera-setup.service
  journalctl -u octessera-setup.service --no-pager
  ```

## Reopen setup later

From local console or an existing admin session:

```sh
sudo rm -f /var/lib/octessera/setup-complete
sudo touch /var/lib/octessera/setup-force
sudo systemctl restart octessera-setup.service
```

Remove the force marker after setup if you used it:

```sh
sudo rm -f /var/lib/octessera/setup-force
```

If Wi-Fi was configured by another Armbian first-run path, the setup portal stays out of the way. Use the force marker above if you still want the Octessera portal.

## Advanced path

Armbian first-run presets still work for fleet or scripted setup. Use those only if you already know how you want to handle Wi-Fi credentials and SSH keys safely.
