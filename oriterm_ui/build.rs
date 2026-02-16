use std::path::PathBuf;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let png_path = workspace_root.join("assets/icon-256.png");

    // Decode PNG to raw RGBA at build time so runtime doesn't need the image crate.
    let png_bytes = std::fs::read(&png_path).expect("read assets/icon-256.png");
    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .expect("decode icon PNG");
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());

    // Format: [u32 width LE][u32 height LE][RGBA pixels...]
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    std::fs::write(format!("{out_dir}/icon_rgba.bin"), &out).expect("write icon_rgba.bin");

    println!("cargo:rerun-if-changed={}", png_path.display());
}
