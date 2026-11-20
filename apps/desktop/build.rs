fn main() {
    // The interface is embedded into the binary: build it first.
    let dist = std::path::Path::new("../../frontend/dist/index.html");
    if !dist.is_file() {
        panic!("frontend is not built: run `cd frontend && npm ci && npm run build` first");
    }
    println!("cargo:rerun-if-changed=../../frontend/dist");
    tauri_build::build();
}
