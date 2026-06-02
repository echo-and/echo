use std::{env, fs, io, path::Path};

fn main() -> io::Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    let icon_dir = Path::new(&manifest_dir).join("assets/images/docker-icons");
    let colors_path = icon_dir.join("colors.json");

    println!("cargo:rerun-if-changed={}", icon_dir.display());
    println!("cargo:rerun-if-changed={}", colors_path.display());

    let mut icons = Vec::new();
    if icon_dir.exists() {
        for entry in fs::read_dir(&icon_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };

            icons.push(stem.to_string());
        }
    }
    icons.sort();
    let colors = read_icon_colors(&colors_path)?;

    let mut generated = String::new();
    generated.push_str("pub(crate) const DOCKER_ICON_ASSETS: &[&str] = &[\n");
    for icon in &icons {
        generated.push_str(&format!("    \"assets/images/docker-icons/{icon}.png\",\n"));
    }
    generated.push_str("];\n\n");

    generated.push_str("pub(crate) const DOCKER_ICON_NAMES: &[&str] = &[\n");
    for icon in &icons {
        generated.push_str(&format!("    \"{icon}\",\n"));
    }
    generated.push_str("];\n\n");

    generated.push_str("pub(crate) const DOCKER_ICON_COLORS: &[(&str, u32)] = &[\n");
    for icon in &icons {
        if let Some(color) = colors
            .iter()
            .find_map(|(name, color)| (name == icon).then_some(color))
        {
            generated.push_str(&format!("    (\"{icon}\", 0x{color:06x}),\n"));
        }
    }
    generated.push_str("];\n\n");

    generated.push_str(
        "pub(crate) fn load_docker_icon_asset(path: &str) -> Option<std::borrow::Cow<'static, [u8]>> {\n",
    );
    generated.push_str("    match path {\n");
    for icon in &icons {
        let asset_path = format!("images/docker-icons/{icon}.png");
        let file_path = icon_dir.join(format!("{icon}.png"));
        let file_path = rust_string_literal_path(&file_path);
        generated.push_str(&format!(
            "        \"{asset_path}\" => Some(std::borrow::Cow::Borrowed(include_bytes!(\"{file_path}\"))),\n",
        ));
    }
    generated.push_str("        _ => None,\n");
    generated.push_str("    }\n");
    generated.push_str("}\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR must be set");
    fs::write(Path::new(&out_dir).join("docker_icons.rs"), generated)
}

fn rust_string_literal_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn read_icon_colors(path: &Path) -> io::Result<Vec<(String, u32)>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path)?;
    let mut colors = Vec::new();

    for line in contents.lines() {
        let line = line.trim().trim_end_matches(',');
        let Some((name, color)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().trim_matches('"');
        let color = color.trim().trim_matches('"').trim_start_matches('#');
        if name.is_empty() || color.len() != 6 {
            continue;
        }
        let Ok(color) = u32::from_str_radix(color, 16) else {
            continue;
        };
        colors.push((name.to_string(), color));
    }

    colors.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(colors)
}
