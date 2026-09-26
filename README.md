# asciiwall

Procedural and image ASCII-art wallpapers for Omarchy (Hyprland), in Rust.

- **Scenes**: `waves`, `plasma`, `noise` (Perlin fBm clouds), `tunnel`, `matrix`, `life` (Conway), `maze` (10 PRINT), plus `image--<name>` — every wallpaper of the current theme converted to ASCII.
- **Shader scenes** (GPU, reinterpreted for ASCII rather than ported): see below.
- **Theme-aware**: glyph colors come from `~/.local/state/omarchy/current/theme/colors.toml`; a `theme-set` hook re-renders on theme change.
- **Static mode**: renders a PNG at the largest monitor's resolution (~100 ms at 4K) and applies it with `omarchy theme bg set`, so the lock screen, transitions and switcher keep working.
- **Live mode**: one wlr-layer-shell surface per monitor on the Bottom layer (above omarchy-shell's background, below windows, click-through), drawn with wgpu on the GPU that drives the monitors. Each frame fills a buffer of (glyph, color) cells — uploaded by CPU scenes, written in place by shader scenes — and a fragment shader expands it through a glyph atlas. Frames are paced by the fps timer *and* the compositor's frame callbacks, so nothing is drawn while a monitor is off; monitors whose active workspace is fullscreen are paused; an unchanged CPU grid is not presented again. A matching static snapshot is still set for the lock screen.
- Rotates to a random enabled scene every `interval_minutes` (default 30).

## Shader scenes

Evaluated once per character cell in a compute shader (256×67 cells on a 4K monitor instead of 8.3 M pixels), so a raymarcher costs a fraction of a millisecond. Most scenes return a field that is sampled at 2×3 points per cell and matched against each glyph's shape (after Alex Harri, ["ASCII characters are not pixels"](https://alexharri.com/blog/ascii-rendering)), so strokes come out as `/ \ | - _ ( )` instead of a brightness ramp. Point scenes scatter tens of thousands of points into that 2×3 grid first. Measured on an RX 6800 at 4K/30 fps: ~0.8 % of one CPU core, 0.1–0.6 ms of GPU per frame.

| scene | inspired by | ASCII take |
|---|---|---|
| `traffic` | [XorDev](https://x.com/XorDev) — [Traffic](https://x.com/XorDev/status/2036188492221305150) | light trails on a winding road as `_.-'` / `|` strokes; cars are `o O 0` pulses |
| `singularity` | XorDev's black holes | tilted orbits drawn as ellipses, lensed arcs over an empty shadow, photon ring in the highlight color |
| `aurora` | XorDev's turbulence ([GM Shaders](https://mini.gmshaders.com)) | ribbon curtains of `| ! :`, a `/\_` ridge, reflections in `~ - =` |
| `fractal` | [Yohei Nishitsuji](https://x.com/YoheiNishitsuji)'s folded fractals | a morphing Sierpinski relic, lit like donut.c with its `.,-~:;=!*#$@` ramp |
| `blossom` | [harshitlog](https://x.com/harshitlog/status/2099485977853231443) | a swaying cherry branch: wood in line glyphs, flowers in `* @ o`, falling petals |
| `koi` | [yuruyurau](https://x.com/yuruyurau)'s point-stream creatures | three koi made of point streams circling a pond with ripple rings |
| `medusa` | yuruyurau | a pulsing jellyfish of ribs, frilled arms and trailing tentacles, marine snow |
| `glass` | harshitlog — "bend images through glass" | lenses drifting over this shader's own source, magnifying whole cells; rims drawn with `_ / | \ -` |

The code is original; only ideas are borrowed. Authors whose tweet shaders are referenced above are credited here — please credit them too if you build on these.

## Install

```sh
cargo build --release
install -Dm755 target/release/asciiwall ~/.local/bin/asciiwall
install -Dm644 contrib/asciiwall.service ~/.config/systemd/user/asciiwall.service
systemctl --user daemon-reload && systemctl --user enable --now asciiwall.service
omarchy hook install theme-set contrib/asciiwall
```

Add the entries from `contrib/omarchy-menu.jsonc` inside `~/.config/omarchy/extensions/omarchy-menu.jsonc`. The extension hot-reloads. SUPER+CTRL+SPACE (Style → Background) keeps opening the native Omarchy image picker with ASCII thumbnails. Style → ASCII Wallpaper adds scene selection, next scene, live/static, rotation (off/15/30/60 minutes) and a config editor. The `theme-set` hook recolors the ASCII wallpaper without changing the selected Omarchy theme. No changes under `/usr/share/omarchy/` or shell plugin are needed.

## Usage

```
asciiwall list                      # scene ids
asciiwall next                      # random scene now
asciiwall set <scene-id|gallery.png>
asciiwall pick                      # Omarchy image picker over the ASCII gallery
asciiwall mode live|static|toggle   # saves config, restarts the service when changed
asciiwall interval <minutes>       # 0 disables rotation; manual next/set still work
asciiwall status mode|interval|scene
asciiwall render <scene> out.png [--width W --height H --font-px P --seed S --time T]
asciiwall gallery                   # re-render picker thumbnails
asciiwall daemon                    # what the systemd unit runs
```

## Config

`~/.config/asciiwall/config.toml` (all keys optional):

```toml
mode = "live"              # "live" | "static"
interval_minutes = 30     # 0 disables automatic rotation
fps = 30
font_family = "JetBrainsMono Nerd Font Mono"
font_size = 16             # em size in logical px
scenes = ["waves", "plasma", "noise", "tunnel", "matrix", "life", "maze", "shaders", "images"]
```

`"images"` expands to every image in `~/.config/omarchy/backgrounds/<theme>/` and the theme's own `backgrounds/`; `"shaders"` to every shader scene.

The menu uses static dotted IDs because the current Omarchy menu only supports its built-in providers; custom scene rows are not dynamically discoverable through JSONC. The existing image picker already supports previews, selection, and keyboard navigation. The CLI dispatcher also scans only packaged `/usr/share/omarchy/bin/omarchy-*` commands, so use `asciiwall` directly rather than modifying packaged scripts. For an upstream integration, Omarchy could optionally add a generic wallpaper-engine interface (choose scene, mode, interval, snapshot for lock/transition) while keeping this renderer independent and the native background service unchanged.

## Uninstall

```sh
systemctl --user disable --now asciiwall.service
rm ~/.config/systemd/user/asciiwall.service ~/.config/omarchy/hooks/theme-set.d/asciiwall ~/.local/bin/asciiwall
```

and remove the `style.background` and `style.asciiwall*` entries from the menu extensions file.
