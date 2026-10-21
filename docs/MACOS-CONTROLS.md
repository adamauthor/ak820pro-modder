# Unified Mac controls

The `0.9.1-macos.1` fork combines the FF13 USB fixes, TFT uploads, keyboard
macros, Mac key remapping and the music knob in **AK820 Pro Modder.app**.
The music code is a Swift static library linked into the Tauri executable.
There is no separate helper application or helper Accessibility permission.

## Use

1. Install the app in Applications and open **Mac controls**.
2. Select **Custom gestures & music** on Knob, then select your preferred service on Music. **System play / pause** retains standard macOS media-key routing.
3. For YouTube Music, install `https://music.youtube.com` using Safari's
   **File → Add to Dock**. Refresh the app list and sign in to YouTube Music.
4. Enable Accessibility for **AK820 Pro Modder** in System Settings, then choose **Stop controls and quit** in the AK820 menu
   and reopen it. Rebuilt ad-hoc-signed versions can require removing and
   re-adding the app's Accessibility entry.
5. Save. **Start with your Mac** is optional. Closing the window or pressing ⌘Q
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
Custom controls require Accessibility; ordinary system Play/Pause does not. This changes the Mac's handling of the knob, not the keyboard's
onboard binding. Turning retains system volume until you enable **Customize turning**.

## Connection scope

| Connection | Identification | Knob service | Key-remap service |
| --- | --- | --- | --- |
| USB | SONiX 0C45:800A, product `AK820` | Consumer page 12 / usage 1 | Keyboard page 1 / usage 6 |
| Bluetooth LE | 0C45:FEFE, product `AK820 5.1-1` | Combined page 1 / usage 6 | Same combined service |

Profiles for Bluetooth names `AK820 5.1-2` and `AK820 5.1-3` are included;
only slot 1 has physical evidence. Unknown devices and ambiguous duplicate
services are left unchanged. A ten-second timer reapplies the knob rule after
reconnection while the app runs.

Mac key mappings are saved once in `mac-controls.json` (`base_remaps`) and
applied by the native controller as the lowest layer under app profiles, on
USB and Bluetooth. The ten-second refresh reapplies them after reconnection,
sleep or restart, together with the knob rule. Builds before 0.9 kept
per-connection backup files; their applied rows are migrated once and the
files are renamed to `*.json.migrated`. Macro shortcuts run while the app is open, including with its
window hidden. USB is still required for lighting, clock, sleep and TFT
configuration. Bluetooth discovery does not enable guessed wireless vendor
commands. FF13 feature limits in [MACOS-800A.md](MACOS-800A.md) remain in force.

## Ownership and lifecycle

The application takes a per-user, nonblocking instance lease before creating a
window or registering controls. Opening a second copy requests the existing
settings window; a duplicate background/login launch exits quietly. Lock file
descriptors are close-on-exec so `hidutil` children cannot retain these leases.

Mapping transactions also take nonblocking leases. Activation notifications
and timer callbacks are coalesced while a refresh is in progress. This matters
because Foundation's process wait can run a nested AppKit loop: waiting on a
second lock in that callback would deadlock the window and every knob action.


- The knob owns Consumer Mute `0C:E2` and optionally Volume Up/Down; each source has a separate original destination.
- Keyboard remapping owns keyboard-page rows. On Bluetooth it preserves the
  consumer rule. Both implementations hold `native-map.lock` around their
  read/modify/write transactions.
- `knob-ownership.json` records original destinations before writes.
  Disabling or using **Stop controls and quit** restores that destination. External conflicting
  changes are detected and left unchanged. Backups survive restarts.
- Custom mode reserves F20, plus F18/F19 with turning enabled, guarded against macro, automation and remap conflicts.
  Its Carbon handler checks its own hotkey signature before handling an event,
  so unrelated automation/macro events pass to their existing handler.
- Revoked Accessibility keeps the YouTube mode F20 rule and reports the missing
  permission; it never falls back to a media key that could launch Music.
  Ordinary Play/Pause mode still uses the normal media key. No global typing listener or audio recording is introduced.
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

## Gestures and application profiles (0.8)

The **Mac controls** workspace now contains Knob, Music, App profiles and
Preferences. RU/EN switches immediately; edits survive tab and language changes.
The saved configuration is authoritative: polling updates status without
replacing an unsaved draft. Save and Revert are shared by all four sections.

Custom mode offers single, double and triple clicks. Single and double clicks
wait for the configurable 220–500 ms inter-click interval; a third click fires
immediately. Holding a click does not count as repeated clicks. Changing the
foreground app discards a pending sequence. A 30-second preview reports physical
gestures without executing them, including a click completed just after the
preview timeout.

Turning customization is opt-in when migrating an existing installation.
Normal turning defaults to output volume; Shift + turn seeks five seconds when
the selected player's Accessibility timeline exposes compatible, writable units.
Unsupported timelines report an error instead of guessing a position. Other
turn actions are Command + =/− zoom, Command + Shift + ]/[ tabs and scrolling.
Application shortcuts require the expected app to remain in front. Individual
apps may use different shortcuts; choose an appropriate action for that app.

Application profiles match an installed app's exact bundle identifier. Their
click/turn actions and optional physical key-to-key overrides apply only while
that app is frontmost; other apps use the default profile. Modder itself always
uses the base mapping. Changes to keyboard overrides wait for held keys to be
released. Profile mappings use the same native lock as base key mappings.

Only the known AJAZZ USB and Bluetooth services are mapped. The consumer Mute
source maps to F20; optional Volume Up/Down sources map to F18/F19. All modifier
combinations of these captured markers are reserved. Macro/automation saves and
keymap edits reject conflicts. Existing unrelated mappings are preserved.
`knob-ownership.json` v2 records each original source destination separately and
migrates the old Mute-only record. Plans are validated before writing, recovery
records precede writes, and writes and rollbacks are read back. Disconnected
services retain recovery records. Full quit restores owned rules; externally
changed rules are preserved rather than overwritten.

## Music services

Choose **YouTube Music**, **Spotify** (native desktop app) or **Apple Music**.
Each card reports installation separately from the saved preference. The optional
“Follow the player that is already playing” setting controls a recognized active
player, remembers that paused player for resume, and otherwise falls back to the
preferred service. Disable it to address the selected app exclusively.

Playback uses identified Accessibility transport controls, or the selected
native player’s documented Apple Events after an explicit Automation grant. No generic media key
is used by custom mode, so an absent or closed YouTube Music window cannot select
Apple Music as macOS's default media handler. A closed window receives an explicit
Launch Services reopen request. YouTube Music retains its confirmed Liked Music
fallback. Spotify and Apple Music resume an existing queue; an empty queue or
sign-in screen requires choosing a track in that app. Their subscriptions and
account access are not managed by Modder. Apple Music and Spotify offer an explicit, optional Automation grant for direct
playback and seeking with closed windows. The application sends fixed Apple
Events to these two applications, without running scripts or handling music
account credentials. YouTube Music retains its Accessibility path.

The standalone **System play / pause** mode intentionally retains the macOS
media-key behavior. Select custom mode for explicit service routing.

The USB protocol, TFT writer, lighting packets and firmware gates are unchanged
by the 0.8 host-controls update. No Bluetooth vendor feature report is invented.


The Music tab includes previous/next, play/pause and five-second seek buttons,
which exercise the same controller used by physical gestures. Native-player
Automation is checked without prompting during status reads. A permission dialog
is requested only by **Allow player control**; revocation reports a clear error.
Already-running players are addressed by PID, so reading another player's state
cannot launch it. YouTube Music's WebKit seek control is an `AXProgressIndicator`
and its spoken elapsed/total description is parsed explicitly (English/Russian).
Native Music/Spotify event codes are taken from their shipped scripting
interfaces; Spotify 1.3.5.3058 was inspected without installation or execution.


## Knob actions

Click actions: play/pause, next, previous, open the player, **mute sound**,
**mute microphone**, **Pomodoro start/pause**, or nothing. Mute actions are
never chosen by default. Volume turns post the system volume keys, so macOS
shows its indicator and unmutes when the level changes; without Accessibility
the volume falls back to Core Audio.

## Updating without losing Accessibility

macOS stores the Accessibility permission against the code signature. Ad-hoc
builds get a new signature every time, so the switch in System Settings stays
on but no longer applies; knob clicks, Telegram detection and YouTube Music
then stop working while volume still changes. Install with a signing
certificate to keep the permission across updates:

```sh
AK820_SIGN_IDENTITY="Apple Development: you@example.com (TEAMID)" scripts/install-mac.sh
```

With an ad-hoc build, remove the app from Accessibility with “−”, add it with
“+”, then choose Stop controls and quit and reopen it. The menu-bar icon shows
⚠︎ and the Smart features tab explains this whenever the permission is missing.
Diagnostics are written to `~/Library/Logs/AK820 Pro Modder/modder.log`.

## Robustness

- Mapping refreshes run on a background queue; AppKit never waits for
  `hidutil`. Recovery records are rewritten only when they change.
- A panic hook restores the original knob and key mappings before the process
  aborts, so a crash cannot leave the knob on F18–F20.
- Recovery records from a newer Modder (version > 2) are refused unchanged.

See [SMART-FEATURES.md](SMART-FEATURES.md) for smart lighting, Pomodoro, the
menu bar and the anime screen.
