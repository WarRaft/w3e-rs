//! Reader and writer for Warcraft III terrain files, `war3map.w3e`: a
//! map's tilesets and, for every point of its grid, the ground height,
//! water, texture, cliffs and flags.
//!
//! ```no_run
//! let bytes = std::fs::read("war3map.w3e").unwrap();
//! let mut terrain = w3e_rs::parse(&bytes).unwrap();
//! let v = terrain.flags_version();
//! let p = terrain.point_mut(10, 20).unwrap();
//! p.set_blight(v, true);
//! std::fs::write("war3map.w3e", terrain.to_bytes().unwrap()).unwrap();
//! ```
//!
//! Reading and writing are exact: `to_bytes` gives back the bytes `parse`
//! read, bits no flag names included, since a point keeps its fields as
//! stored and reads its flags from them. The format, and which game writes
//! which version, is in the README.

pub mod text;

pub use parser_rs::bin::Rawcode;
use parser_rs::bin::{Reader, Writer};
pub use parser_rs::{BinError, BinResult};

/// `W3E!`.
pub const MAGIC: [u8; 4] = *b"W3E!";

/// The height a point at the ground's zero level stores (`0x2000`).
pub const ZERO_HEIGHT: u16 = 0x2000;

/// The distance between two points of the grid, in world units.
pub const CELL: f32 = 128.0;

/// The base tilesets, by the letter the file stores.
pub const TILESETS: [(u8, &str); 18] = [
    (b'A', "Ashenvale"),
    (b'B', "Barrens"),
    (b'K', "Black Citadel"),
    (b'Y', "Cityscape"),
    (b'X', "Dalaran"),
    (b'J', "Dalaran Ruins"),
    (b'D', "Dungeon"),
    (b'C', "Felwood"),
    (b'I', "Icecrown Glacier"),
    (b'F', "Lordaeron Fall"),
    (b'L', "Lordaeron Summer"),
    (b'W', "Lordaeron Winter"),
    (b'N', "Northrend"),
    (b'O', "Outland"),
    (b'Z', "Sunken Ruins"),
    (b'G', "Underground"),
    (b'V', "Village"),
    (b'Q', "Village Fall"),
];

/// The name of the tileset with letter `letter`.
pub fn tileset_name(letter: u8) -> Option<&'static str> {
    TILESETS.iter().find(|(l, _)| *l == letter).map(|(_, n)| *n)
}

/// One point of the grid, its fields as stored. The flags and the fields
/// packed together are read and written through the methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    /// [`ZERO_HEIGHT`] is the ground's zero level.
    pub height: u16,
    /// The water level in the low 14 bits ([`water_height`](Self::water_height)),
    /// the map edge flag in bit 14.
    pub water: u16,
    /// The ground texture and the ramp, blight, water and boundary flags.
    /// One byte up to version 11, two from 12, which gives the texture more
    /// bits; see [`Version`].
    pub flags: u16,
    /// The ground variation in the low 5 bits, the cliff variation in the
    /// high 3.
    pub variation: u8,
    /// The layer (cliff level) in the low 4 bits, the cliff texture in the
    /// high 4.
    pub layer: u8,
}

/// Which bits of [`Point::flags`] are what, by the file's version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// Bits of the ground texture, from bit 0.
    texture_bits: u32,
}

impl Version {
    pub fn of(version: u32) -> Version {
        Version {
            texture_bits: if version >= 12 { 6 } else { 4 },
        }
    }
    /// How many ground textures a point can name.
    pub fn textures(self) -> usize {
        1 << self.texture_bits
    }
    fn texture_mask(self) -> u16 {
        (1 << self.texture_bits) - 1
    }
    fn flag(self, n: u32) -> u16 {
        1 << (self.texture_bits + n)
    }
}

impl Point {
    pub fn water_height(self) -> u16 {
        self.water & 0x3FFF
    }
    pub fn set_water_height(&mut self, h: u16) {
        self.water = (self.water & !0x3FFF) | (h & 0x3FFF);
    }
    /// On the map's edge: the camera bounds' outside.
    pub fn edge(self) -> bool {
        self.water & 0x4000 != 0
    }
    pub fn set_edge(&mut self, on: bool) {
        set_bit16(&mut self.water, 0x4000, on);
    }

    /// The index of the ground texture in [`Terrain::ground`].
    pub fn texture(self, v: Version) -> u16 {
        self.flags & v.texture_mask()
    }
    pub fn set_texture(&mut self, v: Version, t: u16) {
        self.flags = (self.flags & !v.texture_mask()) | (t & v.texture_mask());
    }
    pub fn ramp(self, v: Version) -> bool {
        self.flags & v.flag(0) != 0
    }
    pub fn set_ramp(&mut self, v: Version, on: bool) {
        set_bit16(&mut self.flags, v.flag(0), on);
    }
    pub fn blight(self, v: Version) -> bool {
        self.flags & v.flag(1) != 0
    }
    pub fn set_blight(&mut self, v: Version, on: bool) {
        set_bit16(&mut self.flags, v.flag(1), on);
    }
    /// Water shows here.
    pub fn has_water(self, v: Version) -> bool {
        self.flags & v.flag(2) != 0
    }
    pub fn set_has_water(&mut self, v: Version, on: bool) {
        set_bit16(&mut self.flags, v.flag(2), on);
    }
    /// Unbuildable and unwalkable: the boundary World Editor paints.
    pub fn boundary(self, v: Version) -> bool {
        self.flags & v.flag(3) != 0
    }
    pub fn set_boundary(&mut self, v: Version, on: bool) {
        set_bit16(&mut self.flags, v.flag(3), on);
    }

    pub fn ground_variation(self) -> u8 {
        self.variation & 0x1F
    }
    pub fn set_ground_variation(&mut self, n: u8) {
        self.variation = (self.variation & !0x1F) | (n & 0x1F);
    }
    pub fn cliff_variation(self) -> u8 {
        self.variation >> 5
    }
    pub fn set_cliff_variation(&mut self, n: u8) {
        self.variation = (self.variation & 0x1F) | ((n & 7) << 5);
    }
    /// The cliff level.
    pub fn layer_height(self) -> u8 {
        self.layer & 0x0F
    }
    pub fn set_layer_height(&mut self, n: u8) {
        self.layer = (self.layer & 0xF0) | (n & 0x0F);
    }
    /// The index of the cliff texture in [`Terrain::cliffs`] (15: none).
    pub fn cliff_texture(self) -> u8 {
        self.layer >> 4
    }
    pub fn set_cliff_texture(&mut self, n: u8) {
        self.layer = (self.layer & 0x0F) | ((n & 0x0F) << 4);
    }
}

fn set_bit16(field: &mut u16, bit: u16, on: bool) {
    if on {
        *field |= bit;
    } else {
        *field &= !bit;
    }
}

/// A whole `war3map.w3e`.
#[derive(Debug, Clone, PartialEq)]
pub struct Terrain {
    /// 11 up to 1.31; 12 from Reforged, with more ground textures.
    pub version: u32,
    /// The base tileset's letter ([`TILESETS`]).
    pub tileset: u8,
    /// Whether the map uses custom tilesets (stored as an integer, kept as
    /// is).
    pub custom_tilesets: u32,
    /// The ground textures (`TerrainArt\Terrain.slk`): a point's texture
    /// is an index into it.
    pub ground: Vec<Rawcode>,
    /// The cliff textures (`TerrainArt\CliffTypes.slk`).
    pub cliffs: Vec<Rawcode>,
    /// Points across and up: one more than the map's size in cells.
    pub width: u32,
    pub height: u32,
    /// The world position of point (0, 0), the bottom left corner.
    pub offset_x: f32,
    pub offset_y: f32,
    /// `width * height` points, row by row from the bottom.
    pub points: Vec<Point>,
}

impl Terrain {
    /// How this version stores [`Point::flags`].
    pub fn flags_version(&self) -> Version {
        Version::of(self.version)
    }

    /// The point at column `x`, row `y` (from the bottom left).
    pub fn point(&self, x: u32, y: u32) -> Option<&Point> {
        (x < self.width && y < self.height).then(|| &self.points[(y * self.width + x) as usize])
    }
    pub fn point_mut(&mut self, x: u32, y: u32) -> Option<&mut Point> {
        (x < self.width && y < self.height).then(|| &mut self.points[(y * self.width + x) as usize])
    }

    /// The world position of point (`x`, `y`).
    pub fn world(&self, x: u32, y: u32) -> (f32, f32) {
        (
            self.offset_x + x as f32 * CELL,
            self.offset_y + y as f32 * CELL,
        )
    }

    /// How many points use each ground texture, by index: as long as
    /// [`ground`](Self::ground), or longer when points use an index past it.
    pub fn texture_usage(&self) -> Vec<usize> {
        let v = self.flags_version();
        let mut used = vec![0; self.ground.len()];
        for p in &self.points {
            let t = p.texture(v) as usize;
            if t >= used.len() {
                used.resize(t + 1, 0);
            }
            used[t] += 1;
        }
        used
    }

    /// How many points use each cliff texture, by index (15: none).
    pub fn cliff_usage(&self) -> [usize; 16] {
        let mut used = [0; 16];
        for p in &self.points {
            used[p.cliff_texture() as usize] += 1;
        }
        used
    }

    /// Changes the version, moving every point's flags to its layout.
    /// Fails, changing nothing, when a point has what the new version
    /// can't hold: a texture past 15, or bits no flag names, for 11.
    pub fn set_version(&mut self, version: u32) -> Result<(), String> {
        if !(11..=12).contains(&version) {
            return Err(format!("unknown w3e version {version} (11 and 12 are known)"));
        }
        let (from, to) = (self.flags_version(), Version::of(version));
        if from == to {
            self.version = version;
            return Ok(());
        }
        let known = from.texture_mask() | from.flag(0) | from.flag(1) | from.flag(2) | from.flag(3);
        let mut points = self.points.clone();
        for (i, p) in points.iter_mut().enumerate() {
            let t = p.texture(from);
            if t > to.texture_mask() || p.flags & !known != 0 {
                return Err(format!(
                    "point ({}, {}) has texture {t} or flags version {version} can't hold",
                    i as u32 % self.width.max(1),
                    i as u32 / self.width.max(1)
                ));
            }
            let mut q = *p;
            q.flags = 0;
            q.set_texture(to, t);
            q.set_ramp(to, p.ramp(from));
            q.set_blight(to, p.blight(from));
            q.set_has_water(to, p.has_water(from));
            q.set_boundary(to, p.boundary(from));
            *p = q;
        }
        self.points = points;
        self.version = version;
        Ok(())
    }

    /// The file's bytes.
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        let count = self.width as usize * self.height as usize;
        if self.points.len() != count {
            return Err(BinError::new(
                0,
                format!(
                    "{} points for a {}x{} grid, which has {count}",
                    self.points.len(),
                    self.width,
                    self.height
                ),
            ));
        }
        let mut w = Writer::new();
        w.bytes(&MAGIC);
        w.u32(self.version);
        w.u8(self.tileset);
        w.u32(self.custom_tilesets);
        w.count(self.ground.len())?;
        for &c in &self.ground {
            w.rawcode(c);
        }
        w.count(self.cliffs.len())?;
        for &c in &self.cliffs {
            w.rawcode(c);
        }
        w.u32(self.width);
        w.u32(self.height);
        w.f32(self.offset_x);
        w.f32(self.offset_y);
        let wide = self.version >= 12;
        for p in &self.points {
            w.u16(p.height);
            w.u16(p.water);
            if wide {
                w.u16(p.flags);
            } else {
                w.u8(p.flags as u8);
            }
            w.u8(p.variation);
            w.u8(p.layer);
        }
        Ok(w.into_bytes())
    }
}

/// Reads a `war3map.w3e`. Every byte must be read.
pub fn parse(bytes: &[u8]) -> BinResult<Terrain> {
    let mut r = Reader::new(bytes);
    if r.array::<4>()? != MAGIC {
        return Err(BinError::new(
            0,
            "not a terrain file: it doesn't start with W3E!",
        ));
    }
    let version = r.u32()?;
    if !(11..=12).contains(&version) {
        return Err(BinError::new(
            4,
            format!("unknown w3e version {version} (11 and 12 are known)"),
        ));
    }
    let tileset = r.u8()?;
    let custom_tilesets = r.u32()?;
    let ground = r.list(4, |r| r.rawcode())?;
    let cliffs = r.list(4, |r| r.rawcode())?;
    let width = r.u32()?;
    let height = r.u32()?;
    let offset_x = r.f32()?;
    let offset_y = r.f32()?;
    let wide = version >= 12;
    let size = if wide { 8 } else { 7 };
    let count = width as usize * height as usize;
    if count.checked_mul(size).is_none_or(|n| n > r.remaining()) {
        return Err(r.error(format!(
            "a {width}x{height} grid needs {} bytes of points, {} are left",
            count.saturating_mul(size),
            r.remaining()
        )));
    }
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        let height = r.u16()?;
        let water = r.u16()?;
        let flags = if wide { r.u16()? } else { r.u8()? as u16 };
        points.push(Point {
            height,
            water,
            flags,
            variation: r.u8()?,
            layer: r.u8()?,
        });
    }
    r.finish()?;
    Ok(Terrain {
        version,
        tileset,
        custom_tilesets,
        ground,
        cliffs,
        width,
        height,
        offset_x,
        offset_y,
        points,
    })
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
