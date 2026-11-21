# w3e-rs

Reader and writer for Warcraft III terrain files, `war3map.w3e`: a map's
tilesets and, for every point of its grid, the ground height, water,
ground texture, cliffs and flags. A library and a small CLI; the
[`wc3-mcp`](../wc3-mcp) server's terrain tools are built on it.

Reading and writing are exact: `to_bytes` gives back the bytes `parse`
read, checked on every map of the game and a corpus of custom maps (207
files, about 4 million points). A point keeps its fields as stored and
reads its flags from them, so bits nothing names survive too.

## Format

Little-endian.

| type        | field                                                        |
|-------------|--------------------------------------------------------------|
| char[4]     | `W3E!`                                                       |
| int         | version: 11; 12 in Reforged                                  |
| char        | base tileset, a letter (`L` ...; the game names them in `UI\WorldEditData.txt` `[TileSets]`) |
| int         | custom tilesets: 1 yes, 0 no                                 |
| int         | number of ground textures `a`                                |
| char[4] × a | ground textures (`TerrainArt\Terrain.slk`: `Ldrt` ...)       |
| int         | number of cliff textures `b`                                 |
| char[4] × b | cliff textures (`TerrainArt\CliffTypes.slk`: `CLdi` ...)     |
| int         | width: points across, one more than the map's cells          |
| int         | height: points up                                            |
| float       | X of the bottom left point                                   |
| float       | Y of the bottom left point                                   |
|             | `width × height` points, row by row from the bottom:         |
| u16         | ground height; `0x2000` is zero                              |
| u16         | water height (low 14 bits), map edge flag (bit 14)           |
| u8 / u16    | ground texture and flags, see below                          |
| u8          | ground variation (low 5 bits), cliff variation (high 3)      |
| u8          | cliff level (low 4 bits), cliff texture (high 4; 15 none)    |

The ground texture is an index into the ground list, followed by four
flags: ramp, blight, water, boundary. Version 11 packs them in a byte (4
bits of texture, then the flags: `0x10` ramp, `0x20` blight, `0x40`
water, `0x80` boundary). Version 12 gives the texture 6 bits and takes two
bytes (`0x40` ramp, `0x80` blight, `0x100` water, `0x200` boundary), so a
point is 8 bytes. `Point` reads and writes them for either
(`Terrain::flags_version`).

Version 12 follows how other tools read Reforged maps; none of the maps
here is one, so only the tests check it, not real files.

Neighbouring points are 128 world units apart: point (`x`, `y`) is at the
offset plus 128 × (`x`, `y`) (`Terrain::world`).

## Library

```rust
let mut terrain = w3e_rs::parse(&std::fs::read("war3map.w3e")?)?;
let v = terrain.flags_version();
let p = terrain.point_mut(10, 20).unwrap();
p.set_blight(v, true);
p.set_layer_height(3);
std::fs::write("war3map.w3e", terrain.to_bytes()?)?;
```

Errors are `parser_rs::BinError`, with the byte offset. A grid bigger
than the file's bytes fails before anything is allocated for it.

`Terrain::set_version` moves every point's flags between 11 and 12, and
refuses a point the new version can't hold.

## Text

For people, the header has a text form (`text`): BNI, one `[w3e]`
section, the points left out, and no comments: what the values mean is
`text::hint`'s to say, for an editor to show after them (the wc3-rs
editor opens `.w3e` files this way, with inlay hints). It says which
games write the version, whether custom tilesets are on, each texture of
a list with how many points use it (`text::Usage`, from the file's
points; the names come from the editor, from the game's files), the map's
size in cells, and where it spans in world coordinates.

```ini
[w3e]
version=11
tileset=L
custom_tilesets=0
ground=Ldrt,Ldro,Lgrs
cliffs=CLdi
width=97
height=97
offset_x=-6144
offset_y=-6144
```

Text reads back as a `Header`, which `Header::apply` puts onto the file
it came from, keeping its points: the grid's size can't change, a list of
textures can change as long as every point's texture keeps its place
(replacing a code retextures its points; removing one in use is
refused), and a new version moves the flags. A field left out keeps the
file's value. Unchanged text gives back the exact bytes (all 207 maps of
the corpus).

## CLI

```sh
w3e-rs war3map.w3e          # read and write back: a check
w3e-rs --info war3map.w3e   # tilesets, size, how many points use each texture
```

## Tests

```sh
cargo test
W3E_CORPUS=<folder of map folders> cargo test --release corpus -- --ignored
```
