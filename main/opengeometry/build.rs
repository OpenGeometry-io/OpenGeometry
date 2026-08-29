// main/opengeometry/build.rs
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Указываем, что нужно пересобрать при изменении layers.yaml
    println!("cargo:rerun-if-changed=layers.yaml");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("aia_layers.rs");

    // Читаем YAML
    let yaml_content = match fs::read_to_string("layers.yaml") {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Warning: layers.yaml not found: {}", e);
            // Создаем пустой файл, чтобы компиляция прошла
            fs::write(&dest_path, "// layers.yaml not found\npub fn aia_layer(_kind: &str) -> Option<&'static str> { None }\n")
                .unwrap();
            return;
        }
    };

    // Парсим YAML вручную без serde_yaml (чтобы избежать лишних зависимостей)
    let mut rust_code = String::new();
    rust_code.push_str("// Auto-generated from layers.yaml\n");
    rust_code.push_str("// DO NOT EDIT MANUALLY\n\n");
    rust_code.push_str("use phf::{phf_map, Map};\n\n");
    rust_code.push_str("/// AIA/NCS Layer mapping - compile-time perfect hash map\n");
    rust_code.push_str("pub static AIA_LAYER_MAP: Map<&'static str, &'static str> = phf_map! {\n");

    // Парсим YAML построчно
    let mut current_layer = String::new();
    let mut in_layers = false;
    
    for line in yaml_content.lines() {
        let line = line.trim();
        
        if line == "layers:" {
            in_layers = true;
            continue;
        }
        
        if !in_layers {
            continue;
        }
        
        if line.is_empty() {
            continue;
        }
        
        // Проверяем, что это слой (заканчивается на ':')
        if line.ends_with(':') && !line.starts_with('-') {
            current_layer = line.trim_end_matches(':').trim().to_string();
            continue;
        }
        
        // Проверяем, что это значение (начинается с '-')
        if line.starts_with('-') && !current_layer.is_empty() {
            let value = line.trim_start_matches('-').trim();
            if !value.is_empty() {
                rust_code.push_str(&format!("    \"{}\" => \"{}\",\n", value.to_lowercase(), current_layer));
            }
        }
    }

    rust_code.push_str("};\n\n");
    rust_code.push_str("/// Map entity kind to AIA/NCS layer code. O(1) lookup, no allocations.\n");
    rust_code.push_str("/// Kind must be already normalized to lowercase.\n");
    rust_code.push_str("pub fn aia_layer(kind: &str) -> Option<&'static str> {\n");
    rust_code.push_str("    AIA_LAYER_MAP.get(kind).copied()\n");
    rust_code.push_str("}\n");

    fs::write(&dest_path, rust_code).unwrap();
    println!("cargo:warning=Generated AIA layers at {:?}", dest_path);
}
