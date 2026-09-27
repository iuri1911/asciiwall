# Plates

Classic ASCII art shown as drawn. These files are **not** covered by the
project's license: each keeps its author's license.

| File | Work | Author | License |
|---|---|---|---|
| `lain.txt` | [Serial Experiments Lain ASCII](https://www.deviantart.com/psybear/art/Serial-Experiments-Lain-ASCII-65841237) (2007) | PsyBear | [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/) |
| `rei.txt` | [Rei Ayanami ASCII 3](https://www.deviantart.com/psybear/art/Rei-Ayanami-ASCII-3-65844587) (2007) | PsyBear | [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/) |

The characters are the artist's. PsyBear published the pieces as images, so
they were read back cell by cell: the character grid was measured from the
image (row gaps and column pitch), and each cell was matched against
Liberation Mono, the metric twin of Courier New (`rei`), or
against JetBrains Mono ink shapes where the glyphs are only 4 px wide
(`lain`). A handful of characters may differ from the original where the
image is ambiguous. The transcriptions are shared under the same license,
non-commercially, with this attribution.

A plate's `cell_aspect` in `plate.rs` is the width/height of the font it was
drawn in (Courier New ≈ 0.615); cells are widened to it so the drawing keeps
its proportions in JetBrains Mono.
