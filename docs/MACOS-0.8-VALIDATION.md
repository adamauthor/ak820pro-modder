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

1. Finish the rebuilt app's Accessibility identity refresh when macOS requests Touch ID.
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

## 0.8.0-macos.2 recovery

A live process sample confirmed a main-thread self-deadlock: the workspace
activation observer was waiting for a `hidutil` process, Foundation delivered
a timer inside that wait, and the timer tried to acquire the mapping lock
already held by the outer transaction. The executable remaining alive was
not evidence of a healthy window or working controls.

The fix uses nonblocking close-on-exec file leases, coalesces reentrant
mapping refreshes, and prevents a second app process from claiming controls.
A separate non-UI native process also confirmed that the running app rejects
a second instance lease (`second_instance_claim=0`). Native regression
fixtures check that a second lease immediately reports busy,
that its descriptor cannot leak through exec, and that release allows a new
lease. All 80 workspace tests pass, with one hardware-only test ignored;
Clippy with warnings denied and the production Tauri build also pass.

The installed recovery build opens and responds to UI navigation after timer
and application-activation events. Only the Applications copy is retained as
an executable app; release/rollback artifacts are ZIP archives. The temporary
QA browser profile (including Caps Lock → Escape) was removed from the user's
configuration. Physical playback verification follows the rebuilt app's
macOS Accessibility identity refresh.

### Closed-window playback follow-up

The user confirmed physical output-volume changes after the recovery. A
follow-up found that reopening a surviving YouTube Music process swallowed
its first toggle, and seeking still depended on the player being foreground.
The follow-up build preserves toggle/track/seek intent across reopening,
reopens an existing player without activating it, and tries the seek control's
own increment/decrement actions before guarded, process-targeted keyboard
input. The native contract fixture passes and the release build succeeds.
Actual first-click/background-seek verification remains pending: the computer
use service explicitly reported a locked Mac at the next launch attempt.
