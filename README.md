# spectra

A fast terminal music visualizer for macOS (Apple Silicon + Intel). Single static binary, no runtime deps, works great in Hyper / iTerm2 / Terminal.app / Ghostty / kitty.

```
spectra              # visualize default microphone input
spectra -f song.mp3  # play & visualize an audio file
spectra --list-devices
```

## Visual styles

| key | style       | description                         |
|-----|-------------|-------------------------------------|
| `1` | `bars`      | classic vertical spectrum bars + peaks |
| `2` | `mirror`    | mirrored bars (expanding from center) |
| `3` | `wave`      | oscilloscope waveform                |
| `4` | `spectro`   | scrolling spectrogram heatmap        |
| `5` | `bars-wave` | combined spectrum + oscilloscope     |
| `6` | `blocks`    | chunky block-only bars               |

## Controls

- `space` / `→` / `tab` — next style
- `←` / `shift+tab` — previous style
- `t` — cycle color theme (rainbow / fire / ocean / magma / mono)
- `q` / `esc` / `ctrl+c` — quit

## Install

### Homebrew (recommended)

```sh
brew tap IbrarYunus/spectra
brew install spectra-vis
```

> The formula is called `spectra-vis` (the name `spectra` is taken in homebrew-core
> by an unrelated C++ eigenvalue library). The installed command is still `spectra`.

### From source

```sh
cargo install --path .
```

## Capturing system audio (Spotify, YouTube, anything playing)

spectra can tap system audio directly via **ScreenCaptureKit** (macOS 13+). No virtual audio drivers needed.

```sh
spectra --system
```

**First run**: macOS will prompt for Screen Recording permission for your terminal app (Hyper, iTerm, Terminal.app, etc.). Grant it in **System Settings → Privacy & Security → Screen Recording**, then fully quit and relaunch your terminal. Re-run `spectra --system`. Despite the name, only audio is captured — ScreenCaptureKit is macOS's unified API for both.

You keep hearing audio normally through your speakers/headphones. `excludesCurrentProcessAudio` is enabled so spectra's own output (when using `-f`) doesn't feed back.

### Alternative: BlackHole (older macOS or preference)

If you're on macOS < 13 or prefer a loopback driver, install [BlackHole 2ch](https://github.com/ExistentialAudio/BlackHole):

```sh
brew install --cask blackhole-2ch
```

Create a Multi-Output Device in Audio MIDI Setup combining your speakers + BlackHole, set it as system output, then `spectra -d "BlackHole 2ch"`.

## Options

```
-f, --file <FILE>      Audio file (mp3/wav/flac/ogg/m4a)
-d, --device <DEVICE>  Input device name (see --list-devices)
    --system           Capture system audio via ScreenCaptureKit (macOS 13+)
-s, --style <STYLE>    Initial style: bars|mirror|wave|spectro|bars-wave|blocks
-t, --theme <THEME>    Color theme: rainbow|fire|ocean|mono|magma
    --fps <FPS>        Frames per second (1-120) [default: 60]
    --no-ui            Hide the status bar
```

## Matrix rain font

The `matrix` style uses Thai script glyphs (ก ข ค ด ต ถ ท ธ ๐ ๑ ๒ …) inspired by the Google font **[Pridi](https://fonts.google.com/specimen/Pridi)**. For the richest look, install Pridi and set it as your terminal font (or add it as a fallback after your primary monospace font):

```sh
brew install --cask font-pridi  # via homebrew/cask-fonts (tap if needed)
# or download from https://fonts.google.com/specimen/Pridi and double-click each .ttf
```

Then in Hyper (`~/.hyper.js`), set:

```js
fontFamily: '"Fira Code", "Pridi", monospace',
```

In iTerm2: *Preferences → Profiles → Text → Font*. In Terminal.app: *Settings → Profiles → Text → Font*.

Without Pridi, your terminal falls back to whatever renders Thai — it still works, just less pretty.

## Credits

Built by **[Ibrar Yunus](https://ibraryunus.com)** — Full-Stack AI Engineer & Data Scientist (University of St Andrews, CS Gold Medal).

- Website — [ibraryunus.com](https://ibraryunus.com)
- GitHub — [@IbrarYunus](https://github.com/IbrarYunus)
- LinkedIn — [ibrar-yunus](https://www.linkedin.com/in/ibrar-yunus/)

Run `spectra --credits` for full in-app attribution.

## Build requirements

- Rust (stable)
- macOS 13+ with Xcode Command Line Tools (for `swiftc` — the `--system` path compiles a small Swift shim that wraps ScreenCaptureKit and gets linked as `libspectra_sc.dylib` next to the binary).
