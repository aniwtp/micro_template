use std::{fs, path::Path, process::Command};

/// Collect all `.fbs` files directly under `dir`.
fn collect_fbs(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("fbs") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

fn main() {
    let schema_dir = "flatbuffers";
    let out_root = "src/utils/generated";

    // Always re-run when any file in flatbuffers/ changes.
    println!("cargo:rerun-if-changed={schema_dir}");

    let files = collect_fbs(Path::new(schema_dir));
    if files.is_empty() {
        return;
    }

    // --- Rust ---
    let out_rs = Path::new(out_root);
    fs::create_dir_all(out_rs).unwrap();

    for f in &files {
        let status = Command::new("flatc")
            .arg("--rust")
            .arg("-o")
            .arg(out_rs)
            .arg("-I")
            .arg(schema_dir)
            .arg(f)
            .status()
            .expect("flatc not found — install: cargo install flatbuffers");

        if !status.success() {
            panic!("flatc (--rust) failed on {f}");
        }
    }

    // Generate mod.rs — one module per generated .rs file.
    let mut mods = String::from("#![allow(warnings, clippy::all)]\n\n");
    if let Ok(entries) = fs::read_dir(out_rs) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file()
                && p.extension().and_then(|x| x.to_str()) == Some("rs")
                && p.file_name().unwrap() != "mod.rs"
            {
                let stem = p.file_stem().unwrap().to_string_lossy();
                mods.push_str(&format!("pub mod {stem};\n"));
            }
        }
    }
    fs::write(out_rs.join("mod.rs"), mods).unwrap();

    // --- TypeScript ---
    let ts_root = Path::new("ts");
    let ts_out = ts_root.join("flatbuffers");
    fs::create_dir_all(&ts_out).unwrap();

    for f in &files {
        let status = Command::new("flatc")
            .arg("--ts")
            .arg("--gen-object-api")
            .arg("--gen-all")
            .arg("-o")
            .arg(&ts_out)
            .arg("-I")
            .arg(schema_dir)
            .arg(f)
            .status()
            .expect("flatc not found");

        if !status.success() {
            panic!("flatc (--ts) failed on {f}");
        }
    }

    let tsconfig = ts_root.join("tsconfig.json");
    if tsconfig.exists() {
        let _ = fs::remove_file(ts_out.join("index.ts"));
        match Command::new("npx").arg("--yes").arg("tsc").current_dir(ts_root).status() {
            Ok(s) if s.success() => println!("cargo:info=tsc: compiled ts/flatbuffers/ → ts/dist/"),
            Ok(s) => {
                println!("cargo:warning=tsc exited with code {s}");
                println!("cargo:warning=Run `cd ts && npm install && npm run build`");
            },
            Err(e) => {
                println!("cargo:warning=Could not run tsc: {e}");
                println!("cargo:warning=Install Node.js deps: cd ts && npm install");
            },
        }
    }
}
