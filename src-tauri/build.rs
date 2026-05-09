fn main() {
    compress_lut_files();

    let mut attributes = tauri_build::Attributes::new();

    if is_windows_msvc_target() {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        add_manifest_for_all_artifacts();
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

fn compress_lut_files() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::{Read, Write};

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let luts_out = out_dir.join("luts");
    fs::create_dir_all(&luts_out).expect("Failed to create luts output dir");

    let luts_src = std::path::Path::new("resources/luts");
    if !luts_src.exists() {
        println!("cargo:warning=LUT source directory not found: resources/luts");
        return;
    }

    let entries = fs::read_dir(luts_src).expect("Failed to read LUT source directory");
    for entry in entries {
        let entry = entry.expect("Failed to read dir entry");
        let path = entry.path();
        if path.extension().map(|e| e == "cube").unwrap_or(false) {
            let file_name = path.file_name().unwrap();
            println!("cargo:rerun-if-changed={}", path.display());

            let mut input = fs::File::open(&path).expect("Failed to open LUT file");
            let mut data = Vec::new();
            input.read_to_end(&mut data).expect("Failed to read LUT file");

            let output_path = luts_out.join(format!("{}.gz", file_name.to_string_lossy()));
            let output = fs::File::create(&output_path).expect("Failed to create compressed file");
            let mut encoder = GzEncoder::new(output, Compression::best());
            encoder.write_all(&data).expect("Failed to compress LUT file");
            encoder.finish().expect("Failed to finish compression");
        }
    }
}

fn is_windows_msvc_target() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

fn add_manifest_for_all_artifacts() {
    let manifest = std::env::current_dir()
        .expect("failed to determine build script current directory")
        .join("windows-app-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    println!("cargo:rustc-link-arg=/WX");
}
