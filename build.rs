use nipper::Document;
use regex::Regex;
use std::{env, fs, path::Path};

static METAINFO: &str = include_str!("assets/com.hunterwittenborn.Celeste.metainfo.xml");

fn main() {
    println!("cargo::rustc-check-cfg=cfg(release_mode)");
    println!("cargo::rustc-check-cfg=cfg(bad_environment)");
    println!("cargo::rustc-check-cfg=cfg(missing_environment)");

    // Configure needed variables.
    let release_mode = match env::var("PROFILE").unwrap().as_str() {
        "release" => true,
        "debug" => false,
        _ => unreachable!(),
    };

    // TODO: Remove later, just for testing.
    let release_mode = true;

    if release_mode {
        println!("cargo::rustc-cfg=release_mode");

        match env::var("ENVIRONMENT") {
            Ok(val) => {
                if !Regex::new("^[a-z-]*$").unwrap().is_match(&val) {
                    // println!("cargo::rustc-cfg=bad_environment");
                }
            }
            Err(_) => {
                // println!("cargo::rustc-cfg=missing_environment");
            }
        }
    }

    // Write out build metadata.
    built::write_built_file().unwrap();

    // Compile the CSS file.
    let out_dir = env::var("OUT_DIR").unwrap();
    let css_path = Path::new(&out_dir).join("style.css");
    let scss = grass::from_path("src/style.scss", &grass::Options::default()).unwrap();
    fs::write(css_path, scss).unwrap();

    // Get the release notes for the current version.
    let release_path = Path::new(&out_dir).join("release.xml");
    let metadata = Document::from(METAINFO);
    let release_notes: String = metadata
        .select("release")
        .iter()
        .find(|node| node.attr("version").unwrap() == env!("CARGO_PKG_VERSION").into())
        .unwrap()
        .children()
        .iter()
        .map(|node| node.html().to_string())
        .collect();
    fs::write(release_path, release_notes).unwrap();
}
