//! The text form of a terrain file, for people to read and edit: its
//! header as BNI (INI as the game reads it, `bni-rs`), one `[w3e]`
//! section, without the points. What the values mean (how many points
//! use each texture, where the map ends) isn't in the text: [`hint`] says
//! it, for an editor to show after them.
//!
//! ```ini
//! [w3e]
//! version=11
//! tileset=L
//! custom_tilesets=0
//! ground=Ldrt,Ldro,Lgrs
//! cliffs=CLdi,CLgr
//! width=97
//! height=97
//! offset_x=-6144
//! offset_y=-6144
//! ```
//!
//! Text gives a [`Header`], which [`Header::apply`] puts onto the file it
//! came from: the points stay. So the grid's size can't change, and the
//! texture lists can change only so that every point still has its
//! texture; a new version moves every point's flags. A field left out
//! keeps the file's value. Numbers are written as the shortest decimal
//! that reads back as the same `f32`, so unchanged text gives back the
//! exact bytes.

use bni_rs::Document;
use bni_rs::ast::{Field, Item};
use parser_rs::{ParseError, Span};

use crate::{Rawcode, Terrain};

/// The section's name.
pub const HEADER: &str = "w3e";

/// A point's cliff texture that means none.
pub const NO_CLIFF: usize = 15;

/// The text form of `t`'s header. What the values mean (the tileset's
/// name, how many points use each texture, where the map ends) isn't in
/// it: an editor shows it ([`hint`]).
pub fn to_text(t: &Terrain) -> String {
    let mut out = format!("[{HEADER}]\n");
    out += &format!("version={}\n", t.version);
    out += &format!("tileset={}\n", tileset_text(t.tileset));
    out += &format!("custom_tilesets={}\n", t.custom_tilesets);
    out += &format!("ground={}\n", codes(&t.ground));
    out += &format!("cliffs={}\n", codes(&t.cliffs));
    out += &format!("width={}\nheight={}\n", t.width, t.height);
    out += &format!("offset_x={}\noffset_y={}\n", t.offset_x, t.offset_y);
    out
}

/// What the points of a file say about its header, for an editor's
/// hints: how many points use each ground texture and each cliff texture,
/// by index ([`Terrain::texture_usage`], [`Terrain::cliff_usage`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub ground: Vec<usize>,
    pub cliffs: Vec<usize>,
}

impl Usage {
    pub fn of(t: &Terrain) -> Usage {
        Usage {
            ground: t.texture_usage(),
            cliffs: t.cliff_usage().to_vec(),
        }
    }
}

/// What the header field `key`, whose value is `value`, means, in
/// English, given the rest of the header `header` (key to value) and,
/// when the file was read, its points' [`Usage`]; `name` names a ground
/// (`true`) or cliff texture by code. `None` when there's nothing to say.
///
/// - `version`: which games write it;
/// - `custom_tilesets`: whether the map uses them;
/// - `ground`, `cliffs`: each texture's name and how many points use it;
/// - `width`, `height`: the map's size in cells (one less: points are
///   the cells' corners);
/// - `offset_x`, `offset_y`: where the map spans, in world coordinates.
pub fn hint(
    key: &str,
    value: &str,
    header: &dyn Fn(&str) -> Option<String>,
    usage: Option<&Usage>,
    name: &dyn Fn(bool, &str) -> Option<String>,
) -> Option<String> {
    let number = |v: &str| v.trim().parse::<f64>().ok();
    match key {
        "version" => match value.trim() {
            "11" => Some("up to 1.31".into()),
            "12" => Some("Reforged: more ground textures".into()),
            _ => None,
        },
        "custom_tilesets" => match value.trim() {
            "0" => Some("no custom tilesets".into()),
            "1" => Some("custom tilesets".into()),
            _ => None,
        },
        "ground" | "cliffs" => {
            let ground = key == "ground";
            let used = usage.map(|u| if ground { &u.ground } else { &u.cliffs });
            let parts: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .enumerate()
                .map(|(i, code)| {
                    let label = name(ground, code).unwrap_or_else(|| code.to_string());
                    match used {
                        Some(used) => format!("{label} {}", used.get(i).copied().unwrap_or(0)),
                        None => label,
                    }
                })
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        "width" | "height" => {
            let n = number(value)?;
            (n >= 1.0).then(|| format!("{} cells", n - 1.0))
        }
        "offset_x" | "offset_y" => {
            let (size, word) = if key == "offset_x" { ("width", "across") } else { ("height", "up") };
            let from = number(value)? as f32;
            let points = header(size).and_then(|v| number(&v)).unwrap_or(1.0) as f32;
            let to = from + (points - 1.0).max(0.0) * crate::CELL;
            Some(format!("{from} to {to} {word}"))
        }
        _ => None,
    }
}

/// A letter, or a number for any other byte.
fn tileset_text(letter: u8) -> String {
    if letter.is_ascii_alphabetic() {
        (letter as char).to_string()
    } else {
        letter.to_string()
    }
}

/// A rawcode as the text writes it: its four characters, or `0x` and hex
/// when those would be read as something else (`,`, `"`, `/`, spaces).
fn code_text(c: Rawcode) -> String {
    let plain = c.to_string();
    if plain.len() == 4 && plain.bytes().all(|b| b.is_ascii_graphic() && !b",\"/".contains(&b)) {
        plain
    } else {
        format!("0x{:08X}", c.to_int())
    }
}

fn codes(list: &[Rawcode]) -> String {
    list.iter().map(|&c| code_text(c)).collect::<Vec<_>>().join(",")
}


/// Is `doc` a terrain file's text form: is its first section `[w3e]`?
pub fn is_terrain_text(doc: &Document) -> bool {
    doc.sections.first().is_some_and(|s| s.name.text == HEADER)
}

/// A header read from text: what it sets. `None` keeps the file's value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    pub version: Option<u32>,
    pub tileset: Option<u8>,
    pub custom_tilesets: Option<u32>,
    pub ground: Option<Vec<Rawcode>>,
    pub cliffs: Option<Vec<Rawcode>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub offset_x: Option<f32>,
    pub offset_y: Option<f32>,
}

impl Header {
    /// Puts the header onto `t`, the file the text came from, its points
    /// kept. Fails, changing nothing, when the points don't allow it: a
    /// different grid size, a point whose texture the lists no longer
    /// have, flags the new version can't hold.
    pub fn apply(&self, t: &mut Terrain) -> Result<(), String> {
        let mut new = t.clone();
        if let Some(v) = self.version {
            new.set_version(v)?;
        }
        for (name, set, has) in [
            ("width", self.width, t.width),
            ("height", self.height, t.height),
        ] {
            if let Some(n) = set.filter(|&n| n != has) {
                return Err(format!(
                    "the {name} is {has}, as the file's points are laid out; {n} would need other points"
                ));
            }
        }
        if let Some(c) = self.tileset {
            new.tileset = c;
        }
        if let Some(n) = self.custom_tilesets {
            new.custom_tilesets = n;
        }
        if let Some(x) = self.offset_x {
            new.offset_x = x;
        }
        if let Some(y) = self.offset_y {
            new.offset_y = y;
        }
        if let Some(ground) = &self.ground {
            new.ground = ground.clone();
        }
        if let Some(cliffs) = &self.cliffs {
            new.cliffs = cliffs.clone();
        }
        if let Some((i, n)) = new
            .texture_usage()
            .iter()
            .enumerate()
            .skip(new.ground.len())
            .find(|(_, n)| **n > 0)
        {
            return Err(format!(
                "{n} points use ground texture {i}, and the list has {}: each point keeps its texture's place in the list",
                new.ground.len()
            ));
        }
        if let Some((i, n)) = new.cliff_usage()[..NO_CLIFF]
            .iter()
            .enumerate()
            .skip(new.cliffs.len())
            .find(|(_, n)| **n > 0)
        {
            return Err(format!(
                "{n} points use cliff texture {i}, and the list has {}: each point keeps its texture's place in the list",
                new.cliffs.len()
            ));
        }
        *t = new;
        Ok(())
    }
}

/// What [`from_text`] read: the header when the text has no errors, and
/// every error, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub header: Option<Header>,
    pub errors: Vec<ParseError>,
}

/// Reads the text form back.
pub fn from_text(text: &str) -> Parsed {
    from_bni(&bni_rs::parse(text))
}

/// [`from_text`] of a document already parsed. Its BNI errors come first.
pub fn from_bni(doc: &Document) -> Parsed {
    let mut errors = doc.errors.clone();
    for item in &doc.preamble {
        errors.push(error(
            item_span(item),
            format!("a field outside a section: the file starts with [{HEADER}]"),
        ));
    }
    let Some(section) = doc.sections.first().filter(|s| s.name.text == HEADER) else {
        let span = doc
            .sections
            .first()
            .map_or(Span::new(0, 0, 0, 1), |s| s.name.span);
        errors.push(error(span, format!("the file starts with [{HEADER}]")));
        return Parsed { header: None, errors };
    };
    let mut h = Header::default();
    let mut seen: Vec<String> = Vec::new();
    for item in &section.items {
        let span = item_span(item);
        let Some(key) = item.key.as_ref().map(|k| k.text.trim()) else {
            errors.push(error(span, "a field without a name"));
            continue;
        };
        if seen.iter().any(|k| k == key) {
            errors.push(error(span, format!("`{key}` is set twice")));
            continue;
        }
        seen.push(key.to_string());
        let value = item.value_text();
        let value = value.trim();
        let result = match key {
            "version" => value
                .parse::<u32>()
                .ok()
                .filter(|v| (11..=12).contains(v))
                .map(|v| h.version = Some(v))
                .ok_or_else(|| "the version is 11 (up to 1.31) or 12 (Reforged)".to_string()),
            "tileset" => tileset(value).map(|c| h.tileset = Some(c)),
            "custom_tilesets" => number(value).map(|n| h.custom_tilesets = Some(n)),
            "width" => number(value).map(|n| h.width = Some(n)),
            "height" => number(value).map(|n| h.height = Some(n)),
            "offset_x" => float(value).map(|x| h.offset_x = Some(x)),
            "offset_y" => float(value).map(|y| h.offset_y = Some(y)),
            "ground" | "cliffs" => {
                match list(item, &mut errors) {
                    Some(l) => {
                        if key == "ground" {
                            h.ground = Some(l);
                        } else {
                            h.cliffs = Some(l);
                        }
                        Ok(())
                    }
                    None => Ok(()), // its errors are in
                }
            }
            _ => Err(format!(
                "no field `{key}`: [{HEADER}] has version, tileset, custom_tilesets, ground, cliffs, width, height, offset_x, offset_y"
            )),
        };
        if let Err(message) = result {
            errors.push(error(span, message));
        }
    }
    for extra in &doc.sections[1..] {
        errors.push(error(
            extra.name.span,
            format!("only [{HEADER}]: the points aren't text"),
        ));
    }
    let header = errors.is_empty().then_some(h);
    Parsed { header, errors }
}

/// A letter, or a number for any other byte, as [`tileset_text`] writes
/// it. Which letters are tilesets is the game's to say.
fn tileset(value: &str) -> Result<u8, String> {
    match value.as_bytes() {
        [c] if c.is_ascii_alphabetic() => Ok(*c),
        _ => value
            .parse()
            .map_err(|_| "the tileset is one letter (L, A ...), or a number from 0 to 255".to_string()),
    }
}

fn number(value: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|_| "a whole number, 0 or more".to_string())
}

fn float(value: &str) -> Result<f32, String> {
    value.parse().map_err(|_| "a number".to_string())
}

/// The rawcodes of a list field; `None` when some aren't, each an error.
fn list(item: &Item, errors: &mut Vec<ParseError>) -> Option<Vec<Rawcode>> {
    if item.value_text().trim().is_empty() {
        return Some(Vec::new());
    }
    let before = errors.len();
    let codes: Vec<Rawcode> = item
        .fields
        .iter()
        .filter_map(|f: &Field| {
            let text = f.text();
            Rawcode::parse(text.trim()).or_else(|| {
                errors.push(error(
                    f.span,
                    format!("`{}` isn't a rawcode: four characters, or 0x and eight hex digits", text.trim()),
                ));
                None
            })
        })
        .collect();
    (errors.len() == before).then_some(codes)
}

fn error(span: Span, message: impl Into<String>) -> ParseError {
    let end_col = if span.end_line == span.start_line {
        span.end_col.max(span.start_col + 1)
    } else {
        span.start_col + 1
    };
    ParseError {
        message: message.into(),
        line: span.start_line,
        col: span.start_col,
        end_col,
    }
}

fn item_span(item: &Item) -> Span {
    let start = item.key.as_ref().map_or(item.eq, |k| k.span);
    let end = item.fields.last().map_or(item.eq, |f| f.span);
    Span::new(start.start_line, start.start_col, end.end_line, end.end_col)
}

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
