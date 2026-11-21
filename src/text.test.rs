use super::*;
use crate::{Point, Version, ZERO_HEIGHT};

/// A 3x2 terrain: textures 0 and 1, cliff 0 on one point.
fn small(version: u32) -> Terrain {
    let v = Version::of(version);
    let points = (0..6)
        .map(|i| {
            let mut p = Point { height: ZERO_HEIGHT, layer: 0xF2, ..Point::default() };
            p.set_texture(v, i % 2);
            p.set_blight(v, i == 3);
            if i == 4 {
                p.set_cliff_texture(0);
            }
            p
        })
        .collect();
    Terrain {
        version,
        tileset: b'L',
        custom_tilesets: 0,
        ground: vec![Rawcode(*b"Ldrt"), Rawcode(*b"Lgrs")],
        cliffs: vec![Rawcode(*b"CLdi")],
        width: 3,
        height: 2,
        offset_x: -256.0,
        offset_y: -128.5,
        points,
    }
}

fn header(text: &str) -> Header {
    let parsed = from_text(text);
    assert_eq!(parsed.errors, vec![], "{text}");
    parsed.header.unwrap()
}

#[test]
fn text_reads_back_to_the_same_bytes() {
    for version in [11, 12] {
        let t = small(version);
        let text = to_text(&t);
        let mut back = t.clone();
        header(&text).apply(&mut back).unwrap();
        assert_eq!(back.to_bytes().unwrap(), t.to_bytes().unwrap());
        assert!(is_terrain_text(&bni_rs::parse(&text)));
    }
    assert_eq!(
        to_text(&small(11)),
        "[w3e]\nversion=11\ntileset=L\ncustom_tilesets=0\nground=Ldrt,Lgrs\ncliffs=CLdi\nwidth=3\nheight=2\noffset_x=-256\noffset_y=-128.5\n"
    );
}

#[test]
fn hints_say_what_the_values_mean() {
    let t = small(11);
    let usage = Usage::of(&t);
    let header = |k: &str| match k {
        "width" => Some("3".to_string()),
        "height" => Some("2".to_string()),
        _ => None,
    };
    let named = |ground: bool, code: &str| (ground && code == "Ldrt").then(|| "Dirt".to_string());
    let h = |k: &str, v: &str, u: Option<&Usage>| hint(k, v, &header, u, &named);
    assert_eq!(h("version", "11", None).as_deref(), Some("up to 1.31"));
    assert_eq!(h("custom_tilesets", "1", None).as_deref(), Some("custom tilesets"));
    assert_eq!(h("ground", "Ldrt,Lgrs", Some(&usage)).as_deref(), Some("Dirt 3, Lgrs 3"));
    assert_eq!(h("ground", "Ldrt,Lgrs", None).as_deref(), Some("Dirt, Lgrs"));
    assert_eq!(h("cliffs", "CLdi", Some(&usage)).as_deref(), Some("CLdi 1"));
    assert_eq!(h("cliffs", "", Some(&usage)), None);
    assert_eq!(h("width", "3", None).as_deref(), Some("2 cells"));
    assert_eq!(h("offset_x", "-256", None).as_deref(), Some("-256 to 0 across"));
    assert_eq!(h("offset_y", "-128.5", None).as_deref(), Some("-128.5 to -0.5 up"));
    assert_eq!(h("tileset", "L", None), None);
}

#[test]
fn header_changes_what_it_sets() {
    let mut t = small(11);
    header("[w3e]\ntileset=A\nground=Ldrt,Agrs,Adrt\ncliffs=CAdi\noffset_x=1.5\n")
        .apply(&mut t)
        .unwrap();
    assert_eq!(t.tileset, b'A');
    assert_eq!(t.ground[1], Rawcode(*b"Agrs"));
    assert_eq!(t.cliffs, vec![Rawcode(*b"CAdi")]);
    assert_eq!((t.offset_x, t.offset_y), (1.5, -128.5));
    assert_eq!(t.points, small(11).points);
}

#[test]
fn a_new_version_moves_the_flags() {
    let mut t = small(11);
    header("[w3e]\nversion=12\n").apply(&mut t).unwrap();
    let v = t.flags_version();
    assert_eq!(t.version, 12);
    assert_eq!(t.point(1, 0).unwrap().texture(v), 1);
    assert!(t.point(0, 1).unwrap().blight(v));
    header("[w3e]\nversion=11\n").apply(&mut t).unwrap();
    assert_eq!(t, small(11));

    let mut t = small(12);
    let v = t.flags_version();
    t.point_mut(0, 0).unwrap().set_texture(v, 20);
    t.ground.resize(21, Rawcode(*b"Ldrt"));
    let err = header("[w3e]\nversion=11\n").apply(&mut t).unwrap_err();
    assert!(err.contains("point (0, 0)"), "{err}");
    assert_eq!(t.version, 12);
}

#[test]
fn what_the_points_forbid() {
    let mut t = small(11);
    let err = header("[w3e]\nground=Ldrt\n").apply(&mut t).unwrap_err();
    assert!(err.contains("3 points use ground texture 1"), "{err}");
    let err = header("[w3e]\ncliffs=\n").apply(&mut t).unwrap_err();
    assert!(err.contains("1 points use cliff texture 0"), "{err}");
    let err = header("[w3e]\nwidth=4\n").apply(&mut t).unwrap_err();
    assert!(err.contains("the width is 3"), "{err}");
    assert_eq!(t, small(11));
}

#[test]
fn errors_in_the_text() {
    let errors = |text: &str| -> Vec<String> {
        let p = from_text(text);
        assert!(p.header.is_none());
        p.errors.into_iter().map(|e| format!("{}: {}", e.line, e.message)).collect()
    };
    let e = errors("[w3e]\nversion=13\ntileset=LL\nground=Ldrt,Lg\nheight=-1\nwhat=1\nheight=2\n[more]\n");
    assert_eq!(e.len(), 7, "{e:?}");
    assert!(e[0].starts_with("2: the version is 11"), "{e:?}");
    assert!(e[1].starts_with("3: the tileset is one letter"), "{e:?}");
    assert!(e[2].starts_with("4: `Lg` isn't a rawcode"), "{e:?}");
    assert!(e[3].starts_with("5: a whole number"), "{e:?}");
    assert!(e[4].starts_with("6: no field `what`"), "{e:?}");
    assert!(e[5].starts_with("7: `height` is set twice"), "{e:?}");
    assert!(e[6].starts_with("8: only [w3e]"), "{e:?}");
    assert!(errors("[cameras]\n")[0].contains("starts with [w3e]"));
    assert!(errors("x=1\n[w3e]\n")[0].contains("outside a section"));
}

#[test]
fn awkward_rawcodes_go_as_hex() {
    let mut t = small(11);
    t.ground[0] = Rawcode(*b"a,b/");
    let text = to_text(&t);
    assert!(text.contains("ground=0x612C622F,Lgrs\n"), "{text}");
    let mut back = small(11);
    header(&text).apply(&mut back).unwrap();
    assert_eq!(back, t);
}

/// Every map's terrain, made text and read back, gives the same bytes:
/// `W3E_CORPUS` is a folder of `<map>/war3map.w3e`.
#[test]
#[ignore]
fn corpus() {
    let dir = std::env::var("W3E_CORPUS").expect("W3E_CORPUS");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path().join("war3map.w3e");
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let t = crate::parse(&bytes).unwrap();
        let mut back = t.clone();
        header(&to_text(&t)).apply(&mut back).unwrap();
        assert_eq!(back.to_bytes().unwrap(), bytes, "{}", path.display());
        n += 1;
    }
    assert!(n > 0);
}
