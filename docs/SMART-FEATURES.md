# Smart features (0.9)

Everything here runs inside AK820 Pro Modder while it is open, including with
its window closed (menu bar). Lighting and screen uploads need the USB cable on
the FF13 revision; on Bluetooth the app remembers what should be shown and
applies it when USB returns.

## Smart lighting

Each source is a layer; the highest active one is shown:

1. **Microsoft Teams call** — while a `com.microsoft.teams*` process captures
   audio (Core Audio process objects, macOS 14.2+). Default: Teams purple,
   breathing.
2. **Pomodoro focus** — see below.
3. **App rule** — lighting for the frontmost app (off by default).
4. **Your lighting** — the last configuration applied in the Lighting tab or
   from the menu bar, stored in `lighting-state.json`.

**Telegram** messages flash blue on top of any layer, then the winning layer
returns. New messages are detected from the unread badge of Telegram's Dock
icon through Accessibility (Telegram and Telegram Desktop). Muted chats that
do not change the badge do not flash.

Settings: `smart-lighting.json`. Writes are retried every 5 seconds after a
failure or a USB reconnect.

## Pomodoro

25/5 minutes by default, a long break after 4 rounds. Focus lights the keyboard
red; brightness steps from 5 down to 1 as time runs out, the last minute
breathes and a paused timer breathes slowly. Breaks return the keyboard to the
other layers. Each phase change flashes red three times and posts a
notification. Control it from the Smart features tab, the menu bar or a knob
click bound to **Pomodoro start / pause**. Settings: `pomodoro.json`.

## Anime screen

The `ak820-anime` crate draws 128×128 chibi characters (Sakura, Yuki, Kuro,
Hikari) in code — no assets, no AI service:

- **Greeting card** — palette and greeting by time of day, weekday and date.
- **Weather** — Open-Meteo, keyless. Only the city name you enter is sent.
- **Now playing** — Music or Spotify track with its cover art.
- **Pomodoro** — optional card at each phase change.

Cards can animate (blinking, petals, rain, stars; at most 8 frames). Every
upload replaces the stored animation in the keyboard's flash, so uploads need
an explicit opt-in, automatic refresh is limited to every 15 minutes or more
and track-change cards to one per 5 minutes. CLI: `ak820 anime --out frames/`
or `--upload --allow-legacy`.

## Menu bar

The **AK820** item shows the Pomodoro countdown and battery level. Its menu
switches the knob mode, lighting presets, smart-lighting sources, Pomodoro and
anime cards. Battery comes from the USB feature report or the Bluetooth
Battery Service of the already connected keyboard (Modder never pairs or
connects to anything new). Notifications at 20% and 10%.

## Settings backup and iCloud

System → Sync pushes and pulls a single `settings-bundle.json` with every
settings file. The Smart features tab can export and import the same bundle.
Every file is validated against its type before anything is written.
