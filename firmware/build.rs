use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    embuild::espidf::sysenv::output();

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let partitions_src = manifest_dir.join("partitions.csv");
    if !partitions_src.exists() {
        return;
    }

    let target_dir = manifest_dir.join("target");
    if let Ok(entries) = fs::read_dir(&target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("esp-idf-sys-") {
                    let out_dir = path.join("out");
                    if out_dir.exists() {
                        let dst = out_dir.join("partitions.csv");
                        let _ = fs::copy(&partitions_src, &dst);
                    }
                }
            }
        }
    }
}
