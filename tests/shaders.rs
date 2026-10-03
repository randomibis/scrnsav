//! Validates every WGSL shader in `shaders/` the same way wgpu does at runtime
//! (naga parse + validate), so a broken or mistyped shader fails `cargo test`
//! instead of panicking when the saver launches.

use std::path::Path;

#[test]
fn all_shaders_parse_and_validate() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders");
    let mut checked = 0;

    for entry in std::fs::read_dir(&dir).expect("read shaders/ directory") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
            continue;
        }

        let src = std::fs::read_to_string(&path).expect("read shader file");

        let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| {
            panic!(
                "WGSL parse error in {}:\n{}",
                path.display(),
                e.emit_to_string(&src)
            )
        });

        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("WGSL validation error in {}:\n{e:?}", path.display()));

        checked += 1;
    }

    assert!(checked > 0, "no .wgsl files found in {}", dir.display());
}
