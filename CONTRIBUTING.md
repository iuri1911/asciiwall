# Contributing to asciiwall

asciiwall is an Omarchy wallpaper engine. A contribution is a **pack**, not a
fork of the renderer. The shipped scenes stay until the maintainer curates
them; do not delete a scene in a drive-by change.

## Install on Omarchy

From a checkout:

```sh
./scripts/install.sh
```

That builds the release binary, installs it to `~/.local/bin/asciiwall`,
enables the user service, installs the `theme-set` hook with
`omarchy hook install`, and merges the Style menu override. It does not write
under `/usr/share/omarchy`. Run the same command after `git pull` to update.
`./scripts/install.sh uninstall` removes the integration and leaves your
config; add `--purge` to also delete `~/.config/asciiwall`, the gallery, and
installed packs.

Rust, if missing: `omarchy pkg add rustup && rustup default stable`.

## What you can add

| kind | where it lives | how it shows up |
|---|---|---|
| shader scene | `packs/<id>/` in a review, or `~/.config/asciiwall/packs/<id>/` locally | id is the directory name |
| wallpaper | same directory, `kind = "wallpaper"` | converted with the existing image path |
| theme image | Omarchy's own background directories | already `image--<stem>`; no pack needed |

Pack roots, first match wins:

1. `~/.config/asciiwall/packs/<id>/`
2. `~/.local/share/asciiwall/packs/<id>/`

Built-in ids always win over a pack with the same name. `"packs"` in
`config.toml` adds every valid pack to rotation. Until then a pack is
listable and settable, but not in the automatic rotation. That is the
curation gate: new work does not displace the current set by merely existing.

`examples/packs/drift/` is the specimen. It is not installed and not curated.

## Pack contract

```text
<id>/scene.toml
<id>/scene.wgsl          # kind = shader
<id>/<image>             # kind = wallpaper; name is the file field
```

```toml
id = "drift"             # 2–32 chars, [a-z][a-z0-9-], no "--", no trailing "-"
kind = "shader"          # shader | wallpaper
shaped = true            # shader: true uses fn field, false uses fn cell
contrast = 1.0           # 0.5..=3.0, optional
points = 0               # required iff the source defines fn points(
masks = ["-~=.", "", "", ""]   # four strings, printable ASCII, no space
pulse = 1.0              # 0.25..=4.0; Natural already uses this
font_scale = 1.0         # 0.5..=1.5; wallpapers default to 0.7
license = "MIT"          # MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, CC0-1.0, CC-BY-4.0, Unlicense
author = "Your Name"
credit = "original, or technique after Name (link); formulas are original"
status = "candidate"     # example | candidate | curated — informational
```

Shader source is appended after `prelude.wgsl` (and `shape.wgsl` when
`shaped = true`). It must not declare `@compute`, `@group`, storage or uniform
bindings, `fn main(`, or `fn scatter(`. Those belong to the engine. Binding 3
is reserved for built-in scenes; a pack cannot attach custom data words.

`pulse` is the pace the author considers Natural. Users then pick Slow (×½),
Natural (×1), or Fast (×2) per scene. Do not encode "I want the whole scene
slower" as a dozen magic coefficients; set `pulse`. Keep layer-specific
coefficients only when one motion should differ from the others.

Wallpaper files are jpg, jpeg, png, or webp, at most 25 MiB, and must sit in
the pack directory (no `../`).

## Check

```sh
asciiwall check                         # every built-in shader source
asciiwall check examples/packs/drift    # one pack directory
cargo test
```

`check` does not need a GPU. Before opening a review, also render a frame:

```sh
asciiwall render <id> /tmp/preview.png --width 1920 --height 1080
```

Budget for a scene that is meant to ship: about 0.5 ms of GPU and 1% of one
CPU core at 4K/30 fps on the machine you have. Say what you measured. A pack
that misses the budget can still be merged as a candidate; it should not
become the default.

## Credit and license

The engine is MIT (see `LICENSE`). A pack keeps its own SPDX id, which must be
one of the licenses listed above. Tweet shaders and processing sketches are
almost never licensed for copying. Reimplement the technique, write your own
formulas, and name the person and the link in `credit` and in the README table
if the scene is curated. Do not paste a third-party shader.

## Review

Use the pull-request template. Include the pack directory, `asciiwall check`
output, a preview image, and the credit line. The maintainer decides what
moves from `candidate` to the shipped set; that choice is still open, so a
new scene should arrive as a pack rather than as an edit to `shader::SPECS`.

## Publishing

The repository is private until the maintainer publishes it. Do not add a
remote, a release, or an install URL. When it is public, the install line
stays `./scripts/install.sh` from a checkout; a one-liner can wait until the
URL is real.
