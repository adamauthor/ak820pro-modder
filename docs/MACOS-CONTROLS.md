# Unified Mac controls

The `0.7.0-macos.2` fork combines the FF13 USB fixes, TFT uploads, keyboard
macros, Mac key remapping and the music knob in **AK820 Pro Modder.app**.
The music code is a Swift static library linked into the Tauri executable.
There is no separate helper application or helper Accessibility permission.

## Use

1. Install the app in Applications and open **Mac controls**.
2. Select **Play / pause** or **Play / pause, or start YouTube Music**.
3. For YouTube Music, install `https://music.youtube.com` using Safari's
   **File → Add to Dock**. Refresh the app list and sign in to YouTube Music.
4. Enable Accessibility for **AK820 Pro Modder** in System Settings, then choose **Stop controls and quit** in the AK820 menu
   and reopen it. Rebuilt ad-hoc-signed versions can require removing and
   re-adding the app's Accessibility entry.
5. Save. **Launch at login** is optional. Closing the window or pressing ⌘Q
   keeps controls running in the menu bar, without a Dock icon. Reopen settings
   from AK820 or Applications. **Stop controls and quit** restores the previous
   knob rule and stops the listener. The same command is in the app menu.

The app is a native menu-bar application (`LSUIElement` / accessory activation
policy), and holds a user-initiated background activity to avoid App Nap while
handling shortcuts. System sleep is still allowed. Force Quit really terminates
the process; the login entry starts it again at the next login.

The music action controls YouTube Music directly and resumes its previous song,
or starts **Liked Music** if there is no queue. It can also pause an already
playing Music/Spotify app when an enabled Pause control is found. Other players
are not sent an untargeted media key in YouTube Music mode. Closing the Safari
web-app window sends the next action through Launch Services to reopen it, even
if its process is still running. Core Audio engine activity or an unknown audio
state never routes a YouTube action to the system's default media app.
The Safari app is discovered from its manifest's HTTPS `music.youtube.com`
origin, rather than a machine-specific bundle identifier. Cold launch can
take several seconds. Playback needs a working network and signed-in player.
YouTube Music mode requires macOS 14.2+; ordinary Play/Pause does not require
Accessibility. This changes the Mac's handling of the knob, not the keyboard's
onboard binding. Turning the knob retains volume control.

## Connection scope

| Connection | Identification | Knob service | Key-remap service |
| --- | --- | --- | --- |
| USB | SONiX 0C45:800A, product `AK820` | Consumer page 12 / usage 1 | Keyboard page 1 / usage 6 |
| Bluetooth LE | 0C45:FEFE, product `AK820 5.1-1` | Combined page 1 / usage 6 | Same combined service |

Profiles for Bluetooth names `AK820 5.1-2` and `AK820 5.1-3` are included;
only slot 1 has physical evidence. Unknown devices and ambiguous duplicate
services are left unchanged. A ten-second timer reapplies the knob rule after
reconnection while the app runs.

Mac key mappings use separate USB/Bluetooth backup files. Load the saved draft
and reapply after reconnection; automatic reconnection currently applies to
the knob only. Macro shortcuts run while the app is open, including with its
window hidden. USB is still required for lighting, clock, sleep and TFT
configuration. Bluetooth discovery does not enable guessed wireless vendor
commands. FF13 feature limits in [MACOS-800A.md](MACOS-800A.md) remain in force.

## Ownership and lifecycle

- The knob owns only Consumer Mute `0C:E2`; other mappings are preserved.
- Keyboard remapping owns keyboard-page rows. On Bluetooth it preserves the
  consumer rule. Both implementations hold `native-map.lock` around their
  read/modify/write transactions.
- `knob-ownership.json` records the original Mute destination before writes.
  Disabling or using **Stop controls and quit** restores that destination. External conflicting
  changes are detected and left unchanged. Backups survive restarts.
- YouTube mode reserves F20, guarded against automation and remap conflicts.
  Its Carbon handler checks its own hotkey signature before handling an event,
  so unrelated automation/macro events pass to their existing handler.
- Revoked Accessibility keeps the YouTube mode F20 rule and reports the missing
  permission; it never falls back to a media key that could launch Music.
  Ordinary Play/Pause mode still uses the normal media key. No global typing
  listener or audio recording is introduced. Core Audio activity flags only
  help confirm whether another known player is active.
- `mac-controls.json` lives in the normal app-data directory. Launch at login
  writes `io.github.wsclx.ak820pro-modder.login.plist` in the user's LaunchAgents
  folder, using `/usr/bin/open` and `--background` to open this app through
  Launch Services. There is no shell command template or separate executable.
- Swift requests run on the existing AppKit main thread; playback work uses a
  serial worker and cancellation generations. No second NSApplication or app
  delegate is created.

## Build and verification

Install the project Rust toolchain, Xcode Command Line Tools, Node and pnpm.
`src-tauri/build.rs` compiles the native sources for Cargo's target architecture
on macOS. Non-macOS targets expose an unavailable Mac-controls view.

```sh
pnpm install --frozen-lockfile
pnpm build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo test --workspace --lib
pnpm tauri:build --bundles app
```

Recent pnpm versions require approval for the existing `esbuild` postinstall;
the workspace allows that package only. Dependencies use copying because
hard-link stores can fail in cloud-managed project folders.

Fixtures cover original-rule ownership across refresh and undo, preservation
of combined Bluetooth keyboard/consumer mappings, conflict refusal, playback
decisions with unknown/stale audio state and absent web-app windows, discovery origin validation and login-path
escaping. Existing web and FF13 protocol fixtures are retained.

Previous hardware evidence includes FF13 image/GIF upload and lighting over
USB, plus Bluetooth knob pause/resume, closed-player launch and Liked Music.
The consolidated app's installation verification is recorded in the fork's
release notes; other hardware revisions and Bluetooth slots remain unverified.
