use super::*;

/// A 3x2 terrain in `version`, each point different.
fn small(version: u32) -> Terrain {
    let points = (0..6)
        .map(|i| Point {
            height: ZERO_HEIGHT + i * 10,
            water: 0x4000 * (i % 2) + 0x1F00 + i,
            flags: if version >= 12 { 0x3C0 | i } else { 0xF0 | i },
            variation: (i as u8) << 5 | 3,
            layer: 0xF2,
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
        offset_y: -128.0,
        points,
    }
}

#[test]
fn reads_and_writes_back_both_versions() {
    for (version, size) in [(11, 7), (12, 8)] {
        let t = small(version);
        let bytes = t.to_bytes().unwrap();
        assert_eq!(bytes.len(), 13 + 4 + 8 + 4 + 4 + 16 + 6 * size);
        let back = parse(&bytes).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.to_bytes().unwrap(), bytes);
    }
}

#[test]
fn point_fields() {
    let t = small(11);
    let v = t.flags_version();
    let p = *t.point(2, 1).unwrap(); // the sixth point
    assert_eq!(p.height, ZERO_HEIGHT + 50);
    assert_eq!((p.water_height(), p.edge()), (0x1F05, true));
    assert_eq!(p.texture(v), 5);
    assert!(p.ramp(v) && p.blight(v) && p.has_water(v) && p.boundary(v));
    assert_eq!((p.ground_variation(), p.cliff_variation()), (3, 5));
    assert_eq!((p.layer_height(), p.cliff_texture()), (2, 15));
    assert_eq!(t.world(2, 1), (0.0, 0.0));
    assert!(t.point(3, 0).is_none());

    let mut q = Point::default();
    q.set_texture(v, 0xFF);
    assert_eq!(q.texture(v), 15);
    q.set_blight(v, true);
    q.set_cliff_variation(9);
    q.set_layer_height(7);
    q.set_cliff_texture(3);
    q.set_water_height(0xFFFF);
    q.set_edge(true);
    assert_eq!(
        (q.flags, q.variation, q.layer, q.water),
        (0x2F, 0x20, 0x37, 0x7FFF)
    );
    q.set_edge(false);
    q.set_blight(v, false);
    assert_eq!((q.flags, q.water), (0x0F, 0x3FFF));
}

#[test]
fn version_12_has_six_texture_bits() {
    let v = Version::of(12);
    let mut p = Point::default();
    p.set_texture(v, 40);
    p.set_has_water(v, true);
    assert_eq!(p.flags, 40 | 0x100);
    assert_eq!(p.texture(v), 40);
    assert!(p.has_water(v) && !p.ramp(v));
}

#[test]
fn bad_files() {
    let bytes = small(11).to_bytes().unwrap();
    assert!(
        parse(&bytes[..bytes.len() - 1])
            .unwrap_err()
            .message
            .contains("bytes of points")
    );
    let mut more = bytes.clone();
    more.push(0);
    assert!(parse(&more).is_err());
    let mut v = bytes.clone();
    v[4] = 10;
    assert!(
        parse(&v)
            .unwrap_err()
            .message
            .contains("unknown w3e version 10")
    );
    assert!(parse(b"W3E").is_err());
    assert!(
        parse(b"XXXX\x0b\0\0\0")
            .unwrap_err()
            .message
            .contains("W3E!")
    );
    // A grid too big for the bytes fails before allocating it.
    let mut huge = small(11);
    huge.points.clear();
    huge.width = 1 << 20;
    huge.height = 1 << 20;
    assert!(huge.to_bytes().is_err());
    let mut head = bytes[..bytes.len() - 6 * 7].to_vec();
    let at = head.len() - 16;
    head[at..at + 8].copy_from_slice(&[0, 0, 0, 0x10, 0, 0, 0, 0x10]);
    assert!(parse(&head).is_err());
}

/// Every map folder under `$W3E_CORPUS` (a `war3map.w3e`) reads and
/// writes back byte for byte: `W3E_CORPUS=dir cargo test --release corpus
/// -- --ignored`.
#[test]
#[ignore]
fn corpus() {
    let dir = std::env::var("W3E_CORPUS").expect("set W3E_CORPUS to a folder of map folders");
    let (mut files, mut points) = (0, 0);
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let Ok(bytes) = std::fs::read(entry.path().join("war3map.w3e")) else {
            continue;
        };
        let t = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", entry.path().display()));
        assert!(
            t.to_bytes().unwrap() == bytes,
            "{}: differs after writing",
            entry.path().display()
        );
        files += 1;
        points += t.points.len();
    }
    assert!(files > 0, "no war3map.w3e under {dir}");
    eprintln!("{files} files, {points} points");
}
