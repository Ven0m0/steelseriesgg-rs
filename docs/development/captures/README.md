# USB HID Captures

Store Wireshark `.pcapng` captures here.

## Naming Convention

```
keyboard-<effect>-<device>.pcapng
headset-<operation>-<device>.pcapng
```

## Capture Procedure

1. Install USBPcap and attach to the SteelSeries USB interface
2. In Wireshark, start capture on the USBPcap interface
3. Filter: `usb.transfer_type == 0x01` (interrupt transfers)
4. Perform the operation in GG
5. Save as `.pcapng` to this directory

## Target Operations

### Keyboard (Apex Pro TKL 2023, PID 0x1628)

- [ ] `keyboard-static-red-0x1628.pcapng` — Static Red FF0000
- [ ] `keyboard-static-green-0x1628.pcapng` — Static Green 00FF00
- [ ] `keyboard-static-blue-0x1628.pcapng` — Static Blue 0000FF
- [ ] `keyboard-static-white-0x1628.pcapng` — Static White FFFFFF
- [ ] `keyboard-brightness-0-0x1628.pcapng` — Brightness 0%
- [ ] `keyboard-brightness-50-0x1628.pcapng` — Brightness 50%
- [ ] `keyboard-brightness-100-0x1628.pcapng` — Brightness 100%
- [ ] `keyboard-breathing-0x1628.pcapng` — Breathing effect
- [ ] `keyboard-reactive-0x1628.pcapng` — Reactive effect
- [ ] `keyboard-spectrum-0x1628.pcapng` — Spectrum/rainbow
- [ ] `keyboard-off-0x1628.pcapng` — Lighting disabled
- [ ] `keyboard-actuation-level-0x1628.pcapng` — Actuation point change (if supported)
- [ ] `keyboard-perkey-rainbow-0x1628.pcapng` — Per-key rainbow (to validate 0x23/0x40)

### USB Interface to Monitor

Hypothesis: **MI_04** (vendor-defined, index 4) is the RGB control interface.
Verify by comparing which interface receives `INTERRUPT OUT` transfers.

Connected device path: `USB\VID_1038&PID_1628`
