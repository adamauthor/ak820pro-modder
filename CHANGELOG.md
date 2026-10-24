# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) once it leaves beta.

## [0.9.5-macos.1] — Per-key RGB in the app and colour profiles

- Lighting → custom works on FF13 keyboards: paint keys on the layout and
  each stroke is written to the keyboard (ANSI LED layout; the ISO key
  left of Z has no LED). The last written map is remembered, since the
  firmware cannot report it.
- Colour profiles: save the current paint under a name, apply or delete
  it, import and export profiles as JSON, and cycle through them with a
  global shortcut (e.g. Control+Alt+C). Applying keeps your brightness and
  becomes the base layer for smart lighting.
- Switching to custom right after another effect no longer fails: the
  keyboard is busy for about 2 s after an effect change, and the RGB write
  now waits for it.

## [0.9.4-macos.1] — TFT slot 1 and per-key RGB

- TFT uploads on FF13 firmware target custom slot 1 (`04 72 01`) instead of
  slot 3, after vespovios/ak820pro-modder@3264608: on another ANSI 800A,
  slot 3 showed a static image only briefly among other animation. Not yet
  re-checked on this keyboard.
- Per-key RGB on FF13 firmware (ANSI layout): `ak820 rgb fill` and
  `ak820 rgb rainbow` write the 576-byte colour bank in two acknowledged
  sessions. Protocol and slot table ported from vespovios/ak820pro-modder.

## [0.9.3-macos.1] — Smooth seeking, context screen and new presets

- Seeking: one knob detent is one second, turned as far as you like. Spotify
  and Apple Music seek by the exact second. YouTube Music only seeks from its
  own Shift+arrow shortcut (10 s; Accessibility value changes are ignored), so
  detents add up and every 10 make one jump. The keys go straight to its
  process: no focus change, no player page, no window brought forward. The
  menu-bar item shows the seconds turned (⏩ +7 s).
- Anime screen follows the context: while music plays it shows the current
  track and checks for a new one every 2 minutes; otherwise greeting and
  weather take turns (every 30 minutes by default); Pomodoro cards come first.
- Automatic cards are single-frame by default: the FF13 firmware pauses the
  lighting for the whole image transfer, about 1.5 s instead of 8 s. The
  lighting is redrawn right after each upload.
- Presets rebuilt around the current features: Work (GoLand/DBeaver/Postman
  colours, 50/10 Pomodoro), Deep focus, Calls (click mutes the mic), Music,
  Gaming and Night. Each combines lighting, smart lighting, Pomodoro, anime
  screen and knob gestures; every part can be left out, and personal
  settings (city, upload consent, music service, profiles, mappings) are kept.

## [0.9.2-macos.1] — Incoming Teams calls and Telegram banners

- Incoming Teams calls pulse fast in the Teams colour until answered or
  declined (Accept/Decline buttons via Accessibility, or the small floating
  call window as a fallback); a running call keeps breathing.
- Telegram also flashes for each notification banner in Notification Center,
  so it no longer depends on the Dock unread counter alone.
- Pomodoro anime cards wait for the phase-change flash and retry while the
  keyboard is briefly busy; lighting writes pause during every TFT upload.
- “Open settings” asks macOS for Accessibility directly.
- Flashes and TFT uploads never overlap, and uploads wait 2.5 s after a
  lighting change (the firmware refused image chunks right after a flash).
- YouTube Music now-playing cards get the track artwork from the video id.
- `--command group:value` drives the running app from scripts and Shortcuts,
  e.g. `pomodoro:start`, `anime:now_playing`, `lighting:off`, `test:telegram`
  (`"…/AK820 Pro Modder.app/Contents/MacOS/ak820pro-app" --command …`).
- Knob gestures and seek diagnostics are written to the log.
- YouTube Music in full screen or on another desktop is controlled in place:
  its window is found even when macOS hides it from the window list, so play,
  pause, seek and now-playing cards no longer reopen or need it in front.
- Seeking accepts landing within 2.6 s (playback keeps moving) and waits for
  the player to report each step.
- Telegram banners are counted only in banner-sized windows; the open
  Notification Center list caused repeated flashes that kept the normal
  lighting from returning.
- Pomodoro cards are single-frame (about 2 s to upload instead of 10), and
  Stop replaces the Pomodoro card with the greeting card.

## [0.9.1-macos.1] — Fixes after hardware testing

- Teams call detection ignores the microphone blips Teams makes at launch: a
  call needs 8 s of capture and ends after 5 s of silence. The Teams colour is
  a saturated violet (#7A00FF) because #6264A7 rendered as bluish white.
- After every TFT upload the winning lighting layer is redrawn (the FF13 image
  transfer could leave Pomodoro red off). Automatic anime cards start one
  interval after launch instead of at every start.
- Weather falls back to the Mac's time-zone city when no city is set.
- Now-playing cards read YouTube Music's player bar (title, artist, cover).
- Telegram unread count is read from Launch Services (no Accessibility needed),
  with the Dock as a fallback; Test buttons for Telegram and Teams lighting.
- The menu-bar item is an icon with only the Pomodoro countdown, so it is no
  longer hidden behind the notch; ⚠︎ and a menu entry appear when
  Accessibility is missing, with an explanation in Smart features.
- Lighting view starts from your saved lighting; Smart features shows it.
- Log file in `~/Library/Logs/AK820 Pro Modder/`; `scripts/install-mac.sh`
  signs with a stable identity so Accessibility survives updates.

## [0.9.0-macos.1] — Smart lighting, Pomodoro and anime screen

### Added
- **Smart lighting** with layered priorities (Teams call → Pomodoro → app rule →
  your own lighting): Microsoft Teams calls light the keyboard (detected from
  the Teams process using the microphone, macOS 14.2+), new Telegram messages
  flash blue (Dock badge via Accessibility), optional lighting per app.
- **Pomodoro** timer: red focus lighting that dims as time runs out and breathes
  in the last minute, red flashes and notifications at phase changes, short and
  long breaks, menu-bar countdown, bindable to a knob click.
- **Anime screen**: procedurally drawn chibi characters (Sakura, Yuki, Kuro,
  Hikari) for greeting, weather (Open-Meteo) and now-playing cards (with cover
  art), optional animation, preview in the app, manual or rate-limited
  automatic upload. New `ak820-anime` crate and `ak820 anime` CLI command.
- **Menu bar**: battery level, active profile, Pomodoro countdown and quick
  switches for the knob mode, lighting presets, smart lighting, Pomodoro and
  anime cards. Low-battery notifications at 20% and 10% (USB feature report or
  the Bluetooth Battery Service).
- Knob click actions **Mute sound**, **Mute microphone** and **Pomodoro**
  (available, never selected by default).
- Settings backup: export/import every setting as one JSON file; iCloud sync
  now carries the whole settings bundle, not only automations.
- `ak820 battery` CLI command.

### Changed
- Knob volume uses the system volume keys, so macOS shows its volume indicator,
  uses its own step and unmutes; Core Audio remains the fallback.
- Mac key mappings are a base layer applied by the native controller: they now
  come back automatically after reconnecting, sleep or restart, on USB and
  Bluetooth alike. Old per-connection backups are migrated once.
- Mapping refreshes run off the main thread and only rewrite recovery records
  when they change (previously two file writes every 10 seconds).
- Fast knob turns while seeking are combined into one jump instead of being
  dropped; a recently found seek control is reused for 3 seconds.

### Fixed
- A crash no longer leaves the knob mapped to F18–F20: a panic hook restores
  the original mappings first.
- An unexpected single-instance lock error no longer makes the app exit silently.
- Recording Mac macros with ⌘ no longer loses repeated keys (WebKit omits key-up
  events while ⌘ is held).
- One invalid macro shortcut no longer stops the others from registering; an
  unrecorded macro can be saved as a draft.
- Key remapping view warns about unsaved changes and asks before discarding them.
- Mapping records written by a newer Modder are refused instead of being
  misread.

### Development
- CI runs on every branch and pull request and checks that the Swift self-test
  actually ran.

## [0.8.0-macos.2] — Knob gestures, app profiles and music services

- Single, double and triple knob clicks, Shift+turn, per-app profiles with their
  own key mappings, Spotify and Apple Music control, Russian interface.

## [0.7.0-macos.2] — Background controls and YouTube window recovery

- Run as a menu-bar application without a Dock icon. Both the close button and
  ⌘Q keep keyboard controls/macros running; Stop controls and quit fully exits.
- Retain a background activity so App Nap does not suspend hidden-window input
  handling, while still allowing system sleep.
- Reopen a closed Safari YouTube Music window through Launch Services even if
  its application process is still alive.
- Remove untargeted media events from YouTube mode, including when Accessibility
  is unavailable. Stale audio-engine activity can no longer launch Apple Music.
- Control YT Music directly; pause other already-running Music/Spotify players
  only through an identified, enabled Pause control.

## [0.7.0-macos.1] — Unified macOS fork

- Integrate the Mac music-knob controller directly into the Tauri executable,
  sharing Modder's Accessibility identity and lifecycle.
- Add Mac controls for USB/Bluetooth Play/Pause, YouTube Music cold launch,
  last-track resume, Liked Music fallback and optional launch at login.
- Discover installed Safari YouTube Music apps from their HTTPS manifest origin.
- Keep the app running when its window closes; add a menu-bar reopen/quit menu.
- Add Bluetooth key remapping with per-connection backup and readback. Preserve
  consumer mappings on combined services and share a transaction lock with
  the knob controller. Restore only the owned Mute rule on disable/quit.
- Guard F20 automation/remap conflicts, retain macro hotkey handlers, and show
  Bluetooth connectivity without suggesting that USB configuration is active.
- Keep keyboard firmware operations and confirmed FF13 paths unchanged.
- Make pnpm builds reproducible in cloud-managed project folders and explicitly
  allow the existing esbuild postinstall.

### Included FF13 / USB 800A support

- Detect SONiX AK820 USB 0C45:800A and separate its FF13 feature control / FF68
  4096-byte TFT profile from web-output firmware. Open macOS HID non-exclusively.
- Add opt-in legacy image/GIF uploads with exact descriptor guards, per-chunk
  acknowledgements, shared transaction lock and a supervised writer process.
- Add legacy lighting, local clock sync, four sleep settings and honest battery
  status. Keep unknown onboard commands disabled and hide incompatible controls.
- Add device-scoped Mac key remapping with backup/readback/undo and Mac keyboard
  macros with focused recording, shortcuts, cancellation and Accessibility checks.
- Prevent unsupported preset key-table writes before applying any preset settings;
  avoid side-effect GIF rewrites on the verified legacy lighting path.
- Document hardware evidence and remaining limitations in docs/MACOS-800A.md.

## [Unreleased]

### Added
- **Now-Playing on TFT (Phase 5e part 1).** New host-side polling task in `src-tauri/src/now_playing_tft.rs` reads the current Music.app / Spotify track every 2 s, rasterises title (2 lines, FONT_8X13_BOLD) + artist (FONT_6X10) + album (FONT_5X8, dim) + source footer (FONT_5X8) onto a 128 × 128 RGB565 frame, and uploads via the existing chunked `upload_tft_animation` path. `embedded-graphics 0.8` provides MonoFonts and the `DrawTarget` trait; `src-tauri/src/tft_text.rs` implements a tiny `Framebuffer128` that writes RGB565-LE directly so we don't double-allocate. Word-boundary line-wrap for titles longer than 16 chars; ellipsis-truncation for over-long lines on every row. Cross-platform — non-macOS `now_playing::fetch` returns None and the panel reads `Nothing playing` indefinitely. New Tauri commands `now_playing_tft_start` / `_stop` / `_status` and a toggle in the TFT view. On start, `TftMemory.forget()` runs so side-effecting commands don't restore a stale preset on top of the live readout. [#6](https://github.com/wsclx/ak820pro-modder/issues/6)

### Fixed
- **TFT survives Lighting / Keymap / Macros / Custom-LED / Game-Mode applies.** Mario reported the panel snapping back to its factory animation every time he saved a different tab. Confirmed against the AJAZZ web driver that `SET_LED_EFFECT` and a few siblings send no extra TFT bytes — the reset is firmware-side coupling, not a missing call from our side. Host-side fix: new `TftMemory` state in `src-tauri/src/tft_memory.rs` remembers the last user-requested TFT content (preset id or raw image bytes + fit mode) and a shared `restore_tft_quietly` helper re-uploads it after every `set_*` / `apply_*` command we suspect of triggering the firmware's TFT-reset path. The restore is a free no-op when the memory is empty (the common case for users who haven't opened the TFT view yet). `tft_factory_default` and the new `tft_forget_memory` Tauri command clear the memory so the user can explicitly opt out.

### Added
- **TFT image upload (PNG / JPEG / GIF)** — new `tft_image` module in `ak820-protocol` decodes the file via the `image` crate (default-features off; PNG / JPEG / GIF only, no WebP / AVIF / BMP to keep the binary lean), fits the source to 128 × 128 in one of three modes (Fill = centre-crop edge-to-edge, Contain = letterbox preserving the whole image, Stretch = independent-axis scale), quantises to RGB565 LE, and hands the result to the existing chunked uploader. GIFs become multi-frame `TftAnimation`s with per-frame delays taken from the GIF's own metadata; truncated at 30 frames (the device-reported `tftMaxFrames` budget). MP4 / video out of scope — pulling in ffmpeg deps would double the binary; users decimate to GIF first. New native file-open dialog via `tauri-plugin-dialog`. New Tauri command `apply_tft_image(path, fit)`.
- **TFT Factory Default button** — `tft_factory_default` Tauri command writes `SET_TFT_BUILT_IN_INDEX(0)` so the panel falls back to the firmware's boot-time animation. Useful when an upload looks broken and you want a known-good state.
- **2 TFT diagnostic presets** added to the catalogue: `Diagnostic · Quadrants` (4 colour quadrants with a 16 px grid + centre cross) and `Diagnostic · Border` (4 px white perimeter). Both visualise whether the upload reaches the full 128 × 128 area — first presets to try on a fresh build before the decorative ones.
- **6 new eye-candy presets** replace the previous Color Cycle / Pulse / Scanline / Checkerboard set (which Mario judged "sinnlose Frame-Animationen"): **Mandala** (static 8-fold radial), **Matrix Rain** (green falling trails), **Plasma** (sum-of-sines), **Starfield** (warp-out), **Wave** (sine drifting horizontally), **Spiral** (3-arm rotating). All exercise the full 128 × 128 frame so the upper-half rendering bug is observable on every preset, not just one or two.
- TFT view UI: file picker + fit-mode toggle + factory-default button in the page header. Diagnostic presets get a warn-tone border to visually distinguish them from the decorative set. PageHeader description no longer claims the panel is "above the knob" — it's bottom-right on the AK820 Pro, and the page header isn't where users go to learn keyboard physical layout.

### Known issues
- TFT presets render only in the **upper half** of the panel on Mario's hardware. The web driver explicitly states `AK820 → 128 × 128`, so dimensions aren't the bug. Suspect a pixel-stride or chunk-header validation we haven't decoded yet. The new `Diagnostic · Quadrants` preset is the next data point — Mario's report will tell us which axis is broken.
- **Horizontal noise lines** on some animations. Reported by Mario after the chunk-header magic fix (0x064F → 0x0650) made the panel render full-screen. Cannot reproduce without a hardware screenshot of which preset shows the artefact and where on the panel — likely a chunk-boundary or row-stride issue but waiting on visual evidence before guessing.

### Deferred
- **10 functional / live-stat TFT presets** (Battery, Volume, CPU, Memory, Clock, etc.) — needs a polling architecture + bitmap-font rasteriser to render text onto the panel. Tracked as Phase 5e in HANDOFF.

Planned for 0.8.x
- TFT image upload UI (drag-and-drop GIF / PNG → frame extract → resize / dither → upload). Gated on Phase 5b3 — USB pcap of the official Windows tool's activation sequence.
- Audio-reactive lighting smoothness pass — likely skip the per-chunk `read_response` in `set_many_at` for write-only payloads so the HID pipeline can keep up at >15 fps.
- iCloud Sync expansion: per-record ID-based merge so two machines editing different automations simultaneously don't lose one side; also sync custom-LED paint snapshots and theme/setting preferences.
- Browser-tab media support in Now-Playing (currently only Music.app + Spotify desktop) — **declined for now** on privacy grounds; revisit if a non-invasive surface emerges.
- JIS physical layout once hardware is available for verification.

## [0.7.0-beta] — 2026-05-14

Mid-cycle feature release. Three new top-level capabilities — audio-reactive lighting (Alpha), iCloud-Drive profile sync (Beta), and four additional physical layouts behind a sidebar picker (Beta) — plus two app-wide robustness fixes (formatted error banners; transparent auto-reconnect on HID handle loss) and a polish pass on the keyboard surface (real ISO-Enter L-shape, slot-based nav-column detection).

### Added — alpha
- **Audio-reactive lighting (alpha)** — macOS-only · macOS 13+. New workspace crate `ak820-audio-reactive` wraps Apple's ScreenCaptureKit for system-audio capture and runs a Hann-windowed real FFT (`realfft` 3.5) → bass / mids / highs bands with dB scaling and asymmetric EMA smoothing. New "Spectrum" preset paints the three bands across the keyboard's vertical zones (red / green / blue). New `audio_reactive_start` / `_stop` / `_status` Tauri commands. "Audio-reactive" card sits at the bottom of the Lighting view with a **two-stage opt-in**: a `Locked / Unlocked` toggle that persists in `localStorage` (so contributors don't re-click), and a `Streaming Off / On` toggle that stays disabled until unlocked. The card carries an `Alpha` badge and an amber-bordered `Experimental — use at your own risk` block. **Known issues**: visible flicker on real music because the 10-chunk HID transfer per frame outruns the firmware's per-key RGB pipeline; frame deduplication eliminates the silence-flicker but the real-music smoothness still needs a protocol-layer change. New `ak820 audio meter` CLI subcommand prints band magnitudes + ASCII bars for smoke-testing the pipeline.

### Added — beta
- **Multi-layout support (beta)** — four new physical-layout files alongside the hardware-verified ISO-DE: **ANSI** (US English; structural rewrite with flat 2.25 u Enter and `\|` at slot 97), **ISO-UK** (British English; UK legends + HID 53 grave, slot 97 `# ~`, slot 98 `\ |`), **ISO-ES** (Spanish; `Ñ` at slot 58, `Ç` at slot 97, Spanish modifier captions), **ISO-FR** (French AZERTY; Q↔A and W↔Z position swaps, French legends + captions). All four marked 🧪 **unverified** in their TS docstrings, README, and HANDOFF — slot allocation is hand-derived from the AK820 Pro ISO-DE firmware export without hardware confirmation. Wire-level protocol is layout-agnostic so remapping and lighting still work on every variant; what changes is just the on-screen rendering. New sidebar `<select>` picker driven by a `useLayout()` hook (`src/data/layouts/use-layout.ts`) with persistence in `localStorage["ak820:layout"]`. Keymap + CustomLightingPaint views consume the active layout instead of hard-coding ISO-DE. **JIS deferred** — Japanese boards have additional physical keys (Henkan, Muhenkan, Kana) and a different row-count that needs hardware verification.
- **iCloud-Drive profile sync (beta)** — automations library round-trips through `~/Library/Mobile Documents/com~apple~CloudDocs/ak820pro-modder/`. Plain user-visible folder in Finder (no app-entitlement / provisioning-profile gymnastics, OSS-friendly). New thin Rust transport in `src-tauri/src/icloud_sync.rs` with three Tauri commands: `icloud_sync_status` (probe + remote mtime), `icloud_sync_push` (local → iCloud), `icloud_sync_pull` (iCloud → local, only when iCloud copy is strictly newer). New "iCloud Sync" card in the System view: toggle persists `ak820:icloud-sync-enabled` in `localStorage`, manual `Pull from iCloud` / `Push to iCloud` / `Refresh status` buttons always available. When the toggle is on: app-mount auto-pulls (handled in `App.tsx` before any view fetches), and `Automations` view auto-pushes after every successful save. Conflict resolution is last-write-wins by file mtime; per-record ID-based merge is a follow-up.

### Changed
- **Real ISO-Enter L-shape rendering.** The Keymap view's keyboard surface now honours `h-30` (2 u tall) and per-cap `w-N` width hints in the layout JSON, and the ISO Enter gets a `clip-path` polygon that carves the L-shape notch out of the bottom-left corner — wider top portion sitting above `Ü +~*`, narrower hook extending down over `#'` (DE) / `# ~` (UK) / `Ç` (ES) / `* µ` (FR). Bottom-margin compensation keeps the flex row's intrinsic height at 1 u so the rest of the layout doesn't shift.
- **Slot-based nav-column detection** in the keyboard surface — slots 107 / 105 / 108 (End / PgUp / PgDn) are matched by *slot number* rather than label string, so the right-hand navigation cluster renders correctly on every layout. Previously the matcher was hard-coded to the DE labels `Ende / Bild↑ / Bild↓` and the non-DE labels stayed in the main rows, colliding visually with the ISO Enter's hook.
- **`capStyleFor` refactored** in `src/views/Keymap.tsx` to parse hint classes from a single `widthMap` (adds `w-22` and `w-30` for ANSI's wider Enter / L-Shift) plus dedicated parsers for `h-N`, `mt--N`, and the existing margin hints. Adding a new width is now one map entry instead of another ternary branch.

### Fixed
- **Error banners stop showing `"[object Object]"`.** The Rust `AppError` enum serialises as `{ kind, message }`; views were calling `String(e)` on the rejected promise's payload, which collapsed every object to the literal `[object Object]`. New `src/errors.ts → formatError(e)` handles strings, Error.message, and our `{kind, message}` shape, falling back to `JSON.stringify` before ever stringifying an object directly. All view-level `setErr(String(e))` calls swept to `setErr(formatError(e))`.
- **HID auto-reconnect on stale handle.** `ConnState::with` now retries the closure once if the cached `Connection` returns a stale-handle error (`disconnected` / `Device not found` / `HID error`). Drops the bad handle, re-opens via `ensure_open`, runs the closure again. Net effect: after a USB unplug-and-replug, the very next user action succeeds transparently — no dedicated Reconnect button, no two-click recovery dance.
- **CI hardening for iCloud tests.** Refactored the sync module to take an injectable iCloud root so unit tests pass on machines without iCloud Drive set up (CI runners don't have an iCloud account). 1 fragile smoke test → 6 hermetic tests using per-test tmpdirs keyed by PID + nanosecond nonce.
- **Layout-picker visual alignment**: ANSI L-Shift was rendering at the default 38 px because `w-30` fell through the unrecognised branch; fixed by adding `w-22 → 80 px` to the width map and switching ANSI's slot 64 from `w-30` to `w-22`. Up-arrow position regression (briefly slipped 42 px right over → due to an accidental `mr-15 → undefined` override) restored.

### Documentation
- README roadmap rows 6d / 6f / 8 updated `🛣 planned` → `🧪 alpha / 🧪 beta`. New "Audio-reactive lighting (Alpha — opt-in)" section under Features documents the macOS-13+ requirement, ScreenCaptureKit / TCC behaviour, FFT pipeline, Spectrum preset, known limitations, and the two-stage UI gate.
- HANDOFF § 6.9 expanded with three new entries: § 6.9d cache-poisoning layer behind macos-14 CI failures, § 6.9e the `[object Object]` formatError trap, § 6.9f stale HID handle auto-reconnect rationale, § 6.9g the Swift Concurrency rpath dance for `ak820-audio-reactive` on Command-Line-Tools-only setups.
- HANDOFF § 6.9b rewritten — shipping coverage now distinguishes ISO-DE (✅ hardware-verified) from ANSI/UK/ES/FR (🧪 unverified) and JIS (🛣 hardware-needed).
- CHANGELOG `[Unreleased]` planned-list rolled forward to 0.8.x.

## [0.6.0-beta] — 2026-05-14

Major feature release. Five new top-level capabilities land — Per-key RGB paint surface, macOS Now-Playing reader, Automations library + keyboard-side triggers, cross-cutting Presets, full Light + Dark theming — plus a Factory Defaults rescue path in the Keymap view and a WCAG-AA contrast pass over the entire UI.

### Added
- **Per-key RGB paint-mode editor** in the Lighting view. Selecting the `custom` mode shows a clickable keyboard surface; click-to-paint, brush palette, fill / clear helpers, debounced auto-apply. Wraps the wire-level `set_custom_led` + `apply_lighting(mode=custom)` sequence so the firmware actually renders the buffer.
- **macOS Now-Playing reader**: card in the System view polls Music.app + Spotify desktop every 2 s and shows title / artist / album. Foundation for streaming the track to the TFT display once Phase 5b3 unblocks.
- **Automations library**: new top-level tab. User defines AppleScript / macOS Shortcut / shell-command entries, persisted as JSON in `~/Library/Application Support/io.github.wsclx.ak820pro-modder/`. Run-button executes host-side and shows stdout / stderr / exit-code inline.
- **15-entry starter library** in the Automations tab. First-launch users adopt curated examples (system / files / clipboard / media / web / dev) with one click instead of an empty screen.
- **Keyboard-side automation triggers**: pick "Automations" in the Keymap action picker, click an entry → backend auto-binds an F13–F24 marker via `tauri-plugin-global-shortcut` (Carbon `RegisterEventHotKey` under the hood — no Accessibility permission needed). The selected physical key now fires the automation host-side. Up to 12 automations can be keyboard-triggered simultaneously. Caps bound to automation markers show the automation name on the keyboard surface rather than the raw F-key label.
- **Factory Defaults button** in the Keymap & Knob view header. Pulls the firmware's stored default keymap for the active layer (via `GET_DEFAULT_KEY_MATRIX` cmd 31 / `GET_DEFAULT_FN_KEY_MATRIX` cmd 28) and stages it into the draft. Save commits — a misclick stays in draft state so Discard cleanly undoes it.
- **Presets tab**: new top-level nav entry with 10 curated cross-cutting profiles bundling lighting + sparse keymap overrides + automation seeds. Categories: Gaming (FPS · MMO), Dev (Linux Terminal · Vibe Coder · White Hat), Office (MS365), Creative (Music Production · Writing Focus), Lifestyle (Streaming · Travel battery saver). Each preset shows component badges before apply; the apply modal exposes per-component checkboxes so the user only takes the parts they want. Inline success banner reports exactly what changed. Applied additively — existing automations with the same name are skipped, not overwritten.
- **Light theme** with `prefers-color-scheme` detection, manual Sun / Moon toggle in the sidebar footer, persistence in `localStorage`.

### Changed
- **Foreground contrast pass**: every `fg-*` and `line-*` token lifted to pass WCAG AA on its primary surface (primaries now hit AAA). `fg-3` was previously 2.7 : 1 — a fail for normal text — and is now 4.7 : 1. The whole token system was refactored onto CSS custom properties so the Light theme could re-use the same Tailwind utilities without per-class `dark:` overrides.
- **Type scale**: `xs` from 11.5 px → 12 px, `2xs` from 10.5 px → 11 px, `sm` 13 → 13.5, `base` 14 → 14.5. Detail rows in modals and cards are no longer squinty.

### Documentation
- README, CHANGELOG, PROTOCOL, HANDOFF all swept to match the shipping feature set. HANDOFF gains two new foot-gun entries (§ 6.9c on F13–F24 marker keys silently capturing system-wide, § 6.9d on the macos-14 `cargo` → `rustup-init` shadow that bit CI).

## [0.5.0-beta] — 2026-05-14

First public preview. Five feature phases implemented end-to-end on the AK820 Pro (ISO-DE, firmware 1.07).

### Added
- **Phase 0 — Foundation**: Tauri 2 + React 19 + Vite + Tailwind 3 shell. Rust workspace with `ak820-protocol` library, `ak820-cli` binary, and the Tauri shell crate. HID transport on `usage_page 0xFF68` (control endpoint, 64-byte reports).
- **Phase 1 — Lighting**: 20 effect modes, RGB picker with secondary colour, brightness / speed 0–5, direction (where supported), live apply with debounce.
- **Phase 2 — System**: Device info read-out (firmware, battery, profile slot), sleep-timer presets, game-mode round-trip.
- **Phase 3 — Keymap**: Base + Fn layer editor over a responsive ISO-DE visual surface, 128-slot round-trip via multi-chunk `GET_KEY` / `SET_KEY`, action picker grouped by category (letters / digits / editing / nav / F-keys / modifiers / media / special / macros).
- **Phase 4 — Macros**: Live recorder capturing browser `keydown` / `keyup` events with millisecond delays, action-list editor, 100 slots × 320 bytes, two-phase atomic commit, ActionCatalog integration for assignment to any key.
- **Phase 5a — Per-key RGB (protocol)**: `SET_CUSTOM_LED_DATA` (cmd 36) encoder + decoder, 128 LEDs × 4 B wire layout, `Mode::Custom = 0x80` switch, CLI `ak820 rgb fill / rainbow` hardware-verified.
- **Phase 5b — TFT display (protocol)**: `SET_TFT_USER_ANIMATION` (cmd 80) frame encoder (256-byte header + RGB565 LE pixel stream), bespoke 8-byte per-chunk header, 4096-byte chunk payload, dedicated `Connection::open_tft()` path on the `0xFF67` interface with 4104-byte output reports.
- **Hardening**:
  - Switched `ConnState` from `std::sync::Mutex` to `tokio::sync::Mutex`; every HID-touching Tauri command is now `async fn`. Killed the freeze class observed on System, Macros, and Keymap views.
  - Native macOS menu (App / Edit / View / Window) with `⌘+R`, `⌘⇧R`, `⌘⌥I` shortcuts wired through `tauri::menu::PredefinedMenuItem` and a custom View submenu.
  - Probe poll reads `enumerate()` only (no HID-mutex contention).
  - Project-local `.claude/settings.json` restricts MCPs to `playwright` only, sharply reducing sub-agent prompt size.
- **Documentation**:
  - [`docs/PROTOCOL.md`](docs/PROTOCOL.md) — living byte-level wire docs with AJAZZ JS-source references.
  - [`docs/HANDOFF.md`](docs/HANDOFF.md) — engineering trail, foot-guns, decisions.
  - [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — high-level design + tech-stack rationale.
- **CLI** (`ak820`): `list`, `probe`, `info`, `game-mode get / set-sleep`, `lighting modes / set`, `macros list`, `rgb fill / rainbow`, `hid-descriptors`, `tft solid / cycle / select-index`.
- **26 unit tests** across protocol / frame / keymap / macros / lighting / per-key-RGB / TFT modules.

### Known limitations
- **Physical layout: ISO-DE only.** The Keymap surface in the app is hard-coded to the German ISO (QWERTZ) variant. The wire protocol is layout-agnostic, so lighting / system / per-key RGB / TFT paths work on every AK820 Pro variant — but ANSI / ISO-FR / ISO-ES / ISO-UK / JIS hardware will see mislabelled keys in the Keymap view. Multi-layout support is on the roadmap; the architecture (`src/data/layouts/`) is already prepared to host additional layouts cleanly separated.
- TFT upload runs at wire level but the display still shows the default animation — `SET_TFT_BUILT_IN_INDEX` doesn't appear to switch to user mode on the AK820 Pro firmware we tested. Awaiting USB pcap of the official Windows tool to ground-truth the final activation sequence.
- Per-key RGB has CLI control but no in-app paint UI yet.
- ISO-Enter L-shape renders as a regular rectangle (visual polish only).
- Only the AK820 Pro is supported. Other AJAZZ keyboard models will require their own per-model config.
- Knob (rotary encoder) is firmware-fixed to Volume ± / Mute. Not remappable through the standard protocol — see `docs/HANDOFF.md` § 6.9a for details.

### Documented foot-guns (don't re-discover these)
- Upstream Linux ports (`gohv`, `TaxMachine`) use a wire format the firmware silently ignores on macOS. We RE'd against the official AJAZZ web driver instead.
- `usage_page 0xFF68` is the **control** endpoint; `0xFF67` is the **TFT** endpoint (4104-byte reports). They look identical to hidapi without descriptor inspection.
- Tauri 2 with `devUrl` pointing at Vite hangs WKWebView. Always use `frontendDist` after a static `pnpm build`.
- The Page-type enum has 16 values; `Macro` is **6**, not 4 (4 is `SYSTEM_KEY`). A wrong value silently no-ops.
- Macro wire flags are inverted-looking: `0xB0`/`0x30` is keyboard, `0x90`/`0x10` is mouse.

[Unreleased]: https://github.com/wsclx/ak820pro-modder/compare/v0.7.0-beta...HEAD
[0.7.0-beta]: https://github.com/wsclx/ak820pro-modder/releases/tag/v0.7.0-beta
[0.6.0-beta]: https://github.com/wsclx/ak820pro-modder/releases/tag/v0.6.0-beta
[0.5.0-beta]: https://github.com/wsclx/ak820pro-modder/releases/tag/v0.5.0-beta
