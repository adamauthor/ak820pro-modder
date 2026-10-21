# What works without the cable

Status for the FF13 revision (USB `0C45:800A`, Bluetooth `0C45:FEFE`).

| Feature | Bluetooth | 2.4 GHz dongle | USB cable |
|---|---|---|---|
| Knob gestures, music, mute, Pomodoro timer | ✅ (Mac-side) | ✅ expected (Mac-side) | ✅ |
| Mac key mappings, Mac macros, app profiles | ✅ | ✅ expected | ✅ |
| Teams / Telegram detection, notifications | ✅ (Mac-side) | ✅ | ✅ |
| Keyboard lighting (smart lighting, Pomodoro red) | ❌ not decoded | ❓ untested | ✅ |
| TFT anime cards / GIF upload | ❌ | ❌ vendor tool also requires the cable | ✅ |
| Clock sync, sleep timer, USB battery report | ❌ | ❓ | ✅ |

Everything that runs on the Mac (detection, timers, mappings) does not care
how the keyboard is connected. Writing *to the keyboard* is the limit:

- **Bluetooth.** The keyboard exposes a vendor HID collection (usage page
  `0xFFFF`) next to the normal keyboard services, but its command set is not
  documented. Sending guessed commands to firmware is risky, so Modder does
  not. Decoding it would need a capture of the official tool talking over
  Bluetooth (the Windows tool does not support that).
- **2.4 GHz dongle.** On the web-driver revision the dongle presents the same
  control interface as the cable (`0C45:8009`). If the FF13 dongle exposes the
  FF13 control collection too, lighting can work through it with no code
  changes beyond adding its product ID. Large TFT transfers are not expected
  to work: the vendor tool only offers GIF upload with the cable.
- **Recommended setup:** upload the anime/GIF card once over USB; it stays in
  the keyboard's flash. Then use Bluetooth or the dongle; Modder keeps the
  wanted lighting and applies it the next time USB is connected.

## Checking the dongle

Plug the receiver in, switch the keyboard to 2.4G (Fn+R pairs it), then:

```sh
cargo run -p ak820-cli -- list
```

If an interface with `usage_page=0xff13` appears for the dongle's product ID,
lighting over 2.4 GHz is a small change: add that product ID to the FF13
profile in `crates/ak820-protocol/src/device.rs`.
