# macOS 0.8 validation

This update is an installed preview undergoing physical keyboard verification. It builds on
`98910b4` (0.7.0-macos.2), whose Bluetooth background playback and closed-window
YouTube Music reopening were confirmed by the user.

## Completed locally

- Rust workspace tests (excluding the CLI): **80 passed**, one hardware test
  intentionally ignored. The application tests include native Swift fixtures.
- Native fixtures cover legacy Mute-only recovery migration, multiple owned
  sources, idempotence, undo, external conflicts, lost mappings, click sequences,
  service routing, profile validation, normalized seek timelines, and the spoken
  elapsed/total timeline exposed by the Safari YouTube Music app.
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

## Confirmed on the user's Mac

- Installed the Apple Silicon preview, retaining the previous application and
  configuration for recovery. Refreshed the existing Accessibility identity.
- The user confirmed physical single, double, and triple clicks are distinguished
  correctly over Bluetooth in the action-free gesture preview.
- Apple Music playback and pause are confirmed through its public Apple Events
  interface. Pausing after closing its window succeeds without a default-player
  media-key fallback.
- A paused, locally generated 60-second QA track advances from 0:16 to 0:21 on
  the +5-second action. No subscription or account credential is required for
  this local playback test.
- Native player control uses only the selected, already-running Music or Spotify
  process. The explicit permission button is separate from status checks, which
  never request Automation access.

## Pending before promotion

1. Finish the latest preview's Accessibility refresh after unlocking the Mac.
2. Confirm physical rotation/Shift rotation against a YouTube Music queue, plus
   its spoken seek timeline. Unsupported timelines must report an error rather
   than guessing seconds from percentages.
3. Test browser profile activation, return to the default profile and verify base
   mappings survive. Remove the disposable QA key override after checking undo.
4. Recheck closing the settings window and Command-Q, plus reopening a closed
   YouTube Music window without launching Apple Music.
5. Verify next/previous against the local Apple Music QA queue, then remove only
   the two locally generated QA tracks from its library.
6. Spotify is not installed on this Mac. The public scripting dictionary in the
   official Apple Silicon distribution was inspected; actual Spotify playback
   and seeking have not been physically tested.

No new keyboard firmware command, USB packet format or Bluetooth vendor write
is introduced by this update. These tests do not imply new wireless TFT support.
