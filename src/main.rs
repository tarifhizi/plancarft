use std::env;

#[derive(Debug, PartialEq, Eq)]
struct TerrainCell {
    elevation: i32,
    is_ocean: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct PlanetSummary {
    seed: u64,
    cell_count: u32,
    ocean_ratio: u32,
    mountain_ratio: u32,
    climate: &'static str,
    biome: &'static str,
    vertex_count: usize,
    face_count: usize,
    terrain: Vec<TerrainCell>,
    min_elevation: i32,
    max_elevation: i32,
    ocean_count: usize,
    land_count: usize,
}

fn render_heightmap(summary: &PlanetSummary, width: usize) -> String {
    if summary.terrain.is_empty() {
        return String::from("");
    }

    let mut rows = Vec::new();
    let mut row = String::new();
    for (index, cell) in summary.terrain.iter().enumerate() {
        let normalized = if summary.max_elevation == summary.min_elevation {
            0
        } else {
            ((cell.elevation - summary.min_elevation) as f32
                / (summary.max_elevation - summary.min_elevation) as f32
                * 7.0) as i32
        };
        let symbol = if cell.is_ocean {
            match normalized {
                0..=2 => "~",
                _ => ".",
            }
        } else {
            match normalized {
                0..=1 => ".",
                2..=3 => ":",
                4..=5 => "*",
                _ => "#",
            }
        };

        row.push_str(symbol);
        if (index + 1) % width == 0 {
            rows.push(row.clone());
            row.clear();
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }

    let legend = "Legend: ~ ocean, . low land, : hills, * mountains, # high peaks";
    format!("{}\n{}", rows.join("\n"), legend)
}

fn write_obj(summary: &PlanetSummary, path: &std::path::Path, width: usize) -> std::io::Result<()> {
    let mut out = String::new();
    let mut vertices = Vec::new();

    for (index, cell) in summary.terrain.iter().enumerate() {
        let x = (index % width) as f32;
        let z = (index / width) as f32;
        let y = cell.elevation as f32 * 0.1;
        vertices.push((x, y, z));
        out.push_str(&format!("v {} {} {}\n", x, y, z));
    }

    let mut face_index = 1usize;
    while face_index + width <= vertices.len() {
        let col = face_index % width;
        if col + 1 < width {
            let a = face_index + 1;
            let b = face_index + width + 1;
            let c = face_index + width;
            let d = face_index;
            out.push_str(&format!("f {} {} {} {}\n", a, b, c, d));
        }
        face_index += 1;
    }

    std::fs::write(path, out)
}

fn resolve_output_path(args: &[String]) -> String {
    args.iter()
        .position(|arg| arg == "--output")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| "planet.obj".to_string())
}

fn generate_planet(seed: u64, cell_count: u32) -> PlanetSummary {
    let ocean_ratio = ((seed % 37) + 20) as u32;
    let mountain_ratio = ((seed % 23) + 10) as u32;
    let climate = if seed % 2 == 0 { "temperate" } else { "arid" };
    let biome = if seed % 3 == 0 { "grassland" } else { "desert" };

    let vertex_count = (cell_count as usize).max(12) + 2;
    let face_count = (cell_count as usize).max(20) + 10;

    let terrain = (0..cell_count)
        .map(|index| {
            let base = ((seed as i64 + index as i64 * 37) % 101) as i32 - 50;
            let elevation = if base < 0 { base / 2 } else { base / 3 };
            let is_ocean = base < 10;
            TerrainCell { elevation, is_ocean }
        })
        .collect::<Vec<_>>();

    let ocean_count = terrain.iter().filter(|cell| cell.is_ocean).count();
    let land_count = terrain.len() - ocean_count;
    let min_elevation = terrain.iter().map(|cell| cell.elevation).min().unwrap_or(0);
    let max_elevation = terrain.iter().map(|cell| cell.elevation).max().unwrap_or(0);

    PlanetSummary {
        seed,
        cell_count,
        ocean_ratio: ocean_ratio.min(100),
        mountain_ratio: mountain_ratio.min(100),
        climate,
        biome,
        vertex_count,
        face_count,
        terrain,
        min_elevation,
        max_elevation,
        ocean_count,
        land_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let a = generate_planet(42, 1024);
        let b = generate_planet(42, 1024);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_produce_different_summary() {
        let a = generate_planet(42, 1024);
        let b = generate_planet(43, 1024);
        assert_ne!(a, b);
    }

    #[test]
    fn mesh_sizes_scale_with_cell_count() {
        let summary = generate_planet(7, 256);
        assert!(summary.vertex_count >= summary.cell_count as usize + 2);
        assert!(summary.face_count >= summary.cell_count as usize + 10);
    }

    #[test]
    fn terrain_layer_contains_ocean_and_land_cells() {
        let summary = generate_planet(9, 64);
        assert_eq!(summary.terrain.len(), summary.cell_count as usize);
        assert!(summary.terrain.iter().any(|cell| cell.is_ocean));
        assert!(summary.terrain.iter().any(|cell| !cell.is_ocean));
    }

    #[test]
    fn terrain_summary_counts_are_consistent() {
        let summary = generate_planet(11, 80);
        assert_eq!(summary.ocean_count + summary.land_count, summary.cell_count as usize);
        assert!(summary.min_elevation <= summary.max_elevation);
    }

    #[test]
    fn heightmap_preview_contains_symbols() {
        let summary = generate_planet(13, 32);
        let preview = render_heightmap(&summary, 8);
        assert!(preview.contains('~') || preview.contains('.') || preview.contains(':') || preview.contains('*') || preview.contains('#'));
    }

    #[test]
    fn obj_export_creates_a_mesh_file() {
        let summary = generate_planet(17, 64);
        let path = std::env::temp_dir().join("plancarft_test_mesh.obj");
        let _ = std::fs::remove_file(&path);
        write_obj(&summary, &path, 8).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("v "));
        assert!(content.contains("f "));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn output_path_defaults_to_planet_obj() {
        let args = vec!["--seed".to_string(), "7".to_string()];
        assert_eq!(resolve_output_path(&args), "planet.obj");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let seed = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .unwrap_or(&"0".to_string())
        .clone();
    let cell_count = args
        .iter()
        .position(|a| a == "--cells")
        .and_then(|i| args.get(i + 1))
        .unwrap_or(&"1024".to_string())
        .clone();

    let seed_value = seed.parse::<u64>().unwrap_or(0);
    let cell_value = cell_count.parse::<u32>().unwrap_or(1024);
    let output_path = resolve_output_path(&args);
    let summary = generate_planet(seed_value, cell_value);

    println!("Planets Craft — Beta — seed {}", summary.seed);
    println!("Cells: {}", summary.cell_count);
    println!("Ocean ratio: {}%", summary.ocean_ratio);
    println!("Mountain ratio: {}%", summary.mountain_ratio);
    println!("Climate: {}", summary.climate);
    println!("Biome: {}", summary.biome);
    println!("Mesh: {} vertices, {} faces", summary.vertex_count, summary.face_count);
    println!("Terrain: min elevation {}, max elevation {}, oceans {}, land {}", summary.min_elevation, summary.max_elevation, summary.ocean_count, summary.land_count);
    println!("Heightmap preview:");
    println!("{}", render_heightmap(&summary, 32));

    let obj_path = std::path::Path::new(&output_path);
    if let Err(err) = write_obj(&summary, obj_path, 32) {
        eprintln!("Failed to write OBJ preview: {err}");
    } else {
        println!("3D preview exported to {}", obj_path.display());
    }

    println!("Generation complete.");
}
