//! `w3e-rs`: checks terrain files and describes them. See `usage`.

use std::process::ExitCode;

use w3e_rs::{parse, tileset_name};

fn usage() {
    eprintln!("usage: w3e-rs <war3map.w3e>          check: read and write back");
    eprintln!(
        "       w3e-rs --info <war3map.w3e>   the tilesets, the size, and how often each texture is used"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (info, path) = match args.as_slice() {
        ["--info", path] => (true, *path),
        [path] if !path.starts_with('-') => (false, *path),
        _ => {
            usage();
            return ExitCode::FAILURE;
        }
    };
    let result = std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| parse(&bytes).map(|t| (bytes, t)).map_err(|e| e.to_string()));
    let (bytes, t) = match result {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if !info {
        if t.to_bytes().ok() != Some(bytes) {
            eprintln!("{path}: differs after writing");
            return ExitCode::FAILURE;
        }
        println!(
            "{path}: version {}, {}x{} points",
            t.version, t.width, t.height
        );
        return ExitCode::SUCCESS;
    }
    let tileset = tileset_name(t.tileset).unwrap_or("unknown");
    println!("version\t{}", t.version);
    println!("tileset\t{} ({tileset})", t.tileset as char);
    println!("custom tilesets\t{}", t.custom_tilesets);
    println!("points\t{}x{}", t.width, t.height);
    println!("offset\t{} {}", t.offset_x, t.offset_y);
    let v = t.flags_version();
    let mut used = vec![0usize; t.ground.len().max(64)];
    let mut cliffs = [0usize; 16];
    for p in &t.points {
        if let Some(n) = used.get_mut(p.texture(v) as usize) {
            *n += 1;
        }
        cliffs[p.cliff_texture() as usize] += 1;
    }
    for (i, c) in t.ground.iter().enumerate() {
        println!("ground {i}\t{c}\t{} points", used[i]);
    }
    for (i, c) in t.cliffs.iter().enumerate() {
        println!("cliff {i}\t{c}\t{} points", cliffs[i]);
    }
    ExitCode::SUCCESS
}
