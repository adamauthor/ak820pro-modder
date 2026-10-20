# macOS 0.8 validation

This update is a preview pending physical keyboard verification. It builds on
`98910b4` (0.7.0-macos.2), whose Bluetooth background playback and closed-window
YouTube Music reopening were confirmed by the user.

## Completed locally

- Rust workspace tests (excluding the CLI): **80 passed**, one hardware test
  intentionally ignored. The application tests include native Swift fixtures.
- Native fixtures cover legacy Mute-only recovery migration, multiple owned
  sources, idempotence, undo, external conflicts, lost mappings, click sequences,
  service routing, profile validation and normalized seek timelines.
- Existing protocol tests, including separate legacy/modern TFT paths, pass.
- Rust formatting and Clippy with warnings denied pass.
- TypeScript and production frontend build pass.
- Apple Silicon Tauri `.app` release build passes.
- Browser interface preview: RU/EN switching, service selection, draft retention
  across navigation and language changes, save/revert, profile creation and
  automatic browser zoom defaults, both themes and keyboard tab navigation.
- The minimum application viewport (960 × 640) has no horizontal overflow in
  the music settings. The preview fixture is development-only and excluded from
  the production bundle.

## Pending on the user's Mac

The Mac was locked during this update. The installed 0.7.0-macos.2 application
has not been replaced. Before promoting this preview:

1. Stop old controls from the AK820 menu, install the new application and refresh
   its existing Accessibility entry if macOS invalidates the ad-hoc signature.
2. Confirm physical single/double/triple clicks and Shift + rotation, first in
   gesture preview, then against a playing YouTube Music queue.
3. Confirm output volume and the player's exposed seek timeline. Unsupported
   timelines must report an error rather than guessing seconds from percentages.
4. Test a browser profile, return to the default profile and verify base key
   mappings survive. Test a disposable key override, including full quit/undo.
5. Recheck background control after closing the window and pressing Command-Q,
   plus reopening a closed YouTube Music window without launching Apple Music.
6. Test Apple Music with an available local track/queue. Spotify is not installed
   on this Mac; its native Accessibility adapter has not been physically tested.

No new keyboard firmware command, USB packet format or Bluetooth vendor write
is introduced by this update. These tests do not imply new wireless TFT support.
