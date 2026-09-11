//! Fingerprint for the one authoritative direct-occurrence schema generator.

const GENERATOR_SOURCES: &[&[u8]] = &[
    include_bytes!("main.rs"),
    include_bytes!("model.rs"),
    include_bytes!("validate.rs"),
    include_bytes!("render_ui.rs"),
    include_bytes!("render_manifest.rs"),
];

pub fn generator_hash() -> String {
    let mut hasher = blake3::Hasher::new();
    for source in GENERATOR_SOURCES {
        hasher.update(source);
    }
    hasher.finalize().to_hex().to_string()
}
