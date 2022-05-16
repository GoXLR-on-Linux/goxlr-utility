# GoXLR Initialiser

This program is designed to prepare a freshly powered on GoXLR for use. It's intended to be run as
early in the boot process as possible, and will iterate over all GoXLR devices, check their state, and if 
needed, activate and enable its features. This is especially useful for the GoXLR Mini device, which loses
power on reboot.

This application will (if necessary) temporarily detach the kernel module and thus the GoXLRs sound components,
so it's important to ensure it's not executed after user-space audio applications and setups (such as Jack or Pulse),
as restoring audio after this point can become difficult.

An example systemd service to activate this would look something like:

```
[Unit]
Description=GoXLR Initialiser

[Service]
Type=oneshot
ExecStart=/path/to/goxlr-initialiser

[Install]
WantedBy=graphical.target
```

Once run, the daemon should be able to start up correctly, and handle profile loading.