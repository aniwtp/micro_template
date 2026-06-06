use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

fn collect_fbs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("fbs") {
                out.push(path);
            }
        }
    }
    out
}

fn hash_file(path: &Path) -> String {
    let data = fs::read(path).expect("failed to read fbs");
    blake3::hash(&data).to_hex().to_string()
}

fn load_cache(path: &Path) -> HashMap<String, String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(path: &Path, cache: &HashMap<String, String>) {
    let json = serde_json::to_string_pretty(cache).unwrap();
    fs::File::create(path)
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
}

fn write_mod_tree(out_root: &Path) {
    fn gen_mod(dir: &Path) {
        let mut mods = String::from("#![allow(warnings, clippy::all)]\n\n");
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.is_file()
                    && path.extension().and_then(|x| x.to_str()) == Some("rs")
                    && path.file_name().unwrap() != "mod.rs"
                {
                    let stem = path.file_stem().unwrap().to_string_lossy();
                    mods.push_str(&format!("pub mod {};\n", stem));
                }
            }
        }
        fs::write(dir.join("mod.rs"), mods).unwrap();
    }

    let dto_dir = out_root.join("dto");
    let types_dir = out_root.join("types");

    if dto_dir.exists() {
        gen_mod(&dto_dir);
    }
    if types_dir.exists() {
        gen_mod(&types_dir);
    }

    fs::write(
        out_root.join("mod.rs"),
        "#![allow(warnings, clippy::all)]\n\npub mod dto;\npub mod types;\n",
    )
    .unwrap();
}

fn patch_generated(out_root: &Path) {
    // Собираем маппинг: "tokens_generated" -> "crate::generated::types::tokens_generated"
    let mut name_to_path: HashMap<String, String> = HashMap::new();

    for subdir in &["dto", "types"] {
        let dir = out_root.join(subdir);
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) == Some("rs")
                    && path.file_name().unwrap() != "mod.rs"
                {
                    let stem = path.file_stem().unwrap().to_string_lossy().to_string();
                    let full = format!("crate::generated::{}::{}", subdir, stem);
                    name_to_path.insert(stem, full);
                }
            }
        }
    }

    // Патчим файлы: правим пути + добавляем allow атрибут
    for subdir in &["dto", "types"] {
        let dir = out_root.join(subdir);
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|x| x.to_str()) == Some("rs")
                    && path.file_name().unwrap() != "mod.rs"
                {
                    let mut content = fs::read_to_string(&path).unwrap();

                    // Фиксим crate:: пути
                    for (short, full) in &name_to_path {
                        let old = format!("crate::{}", short);
                        content = content.replace(&old, full);
                    }

                    // Добавляем allow в начало
                    content = format!("#![allow(warnings, clippy::all)]\n\n{}", content);

                    fs::write(&path, content).unwrap();
                }
            }
        }
    }
}

fn main() {
    let schema_root = PathBuf::from("flatbuffers");
    let out_root = PathBuf::from("src/generated");

    let cache_file = PathBuf::from(env::var("OUT_DIR").unwrap()).join("fbs_hash.json");

    let dto_dir = schema_root.join("dto");
    let types_dir = schema_root.join("types");

    let all_files: Vec<_> = collect_fbs(&types_dir)
        .into_iter()
        .chain(collect_fbs(&dto_dir))
        .collect();

    println!("cargo:rerun-if-changed=flatbuffers");
    for file in &all_files {
        println!("cargo:rerun-if-changed={}", file.display());
    }

    let old_cache = load_cache(&cache_file);
    let mut new_cache = HashMap::new();
    let mut changed = false;

    for file in &all_files {
        let key = file.to_string_lossy().to_string();
        let hash = hash_file(file);
        if old_cache.get(&key) != Some(&hash) {
            changed = true;
        }
        new_cache.insert(key, hash);
    }

    if !changed {
        return;
    }

    fs::create_dir_all(&out_root).unwrap();

    for file in &all_files {
        let relative = file.strip_prefix(&schema_root).unwrap();
        let out_subdir = out_root.join(relative.parent().unwrap_or(Path::new("")));
        fs::create_dir_all(&out_subdir).unwrap();

        let status = Command::new("flatc")
            .arg("--rust")
            .arg("-o")
            .arg(&out_subdir)
            .arg("-I")
            .arg(&dto_dir)
            .arg("-I")
            .arg(&types_dir)
            .arg(file)
            .status()
            .expect("flatc not found");

        if !status.success() {
            panic!("flatc failed on {:?}", file);
        }
    }

    write_mod_tree(&out_root);
    patch_generated(&out_root);
    save_cache(&cache_file, &new_cache);
}
