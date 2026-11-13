use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use rift_cache::{Cache, Model, ObjectDefinition, Terrain};

#[derive(Parser, Debug)]
#[command(name = "rift-tools", about = "OSRS cache inspection for Rift")]
struct Cli {
    /// Cache directory containing main_file_cache.dat2.
    #[arg(long, default_value = ".cache/osrs-235/cache", global = true)]
    cache: PathBuf,

    /// XTEA keys JSON. Defaults to keys.json next to the cache.
    #[arg(long, global = true)]
    keys: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print cache index and reference table stats.
    CacheInfo,
    /// Decode terrain for a region.
    Terrain { region_x: u32, region_y: u32 },
    /// Decode placed locations for a region.
    Locations { region_x: u32, region_y: u32 },
    /// Print an underlay definition.
    Underlay { id: u32 },
    /// Print an overlay definition.
    Overlay { id: u32 },
    /// Print an object definition.
    Object { id: u32 },
    /// Search object definitions by name substring.
    FindObject {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Print a model summary.
    Model { id: u32 },
    /// Export a model as a Wavefront OBJ.
    ExportObj { id: u32, out: PathBuf },
    /// Export terrain height/colour data as an OBJ for quick inspection.
    TerrainObj {
        region_x: u32,
        region_y: u32,
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let cache = Cache::open_with_keys(&cli.cache, cli.keys.as_ref())
        .with_context(|| format!("opening cache at {}", cli.cache.display()))?;

    match cli.command {
        Command::CacheInfo => cache_info(&cache)?,
        Command::Terrain { region_x, region_y } => terrain(&cache, region_x, region_y)?,
        Command::Locations { region_x, region_y } => locations(&cache, region_x, region_y)?,
        Command::Underlay { id } => underlay(&cache, id)?,
        Command::Overlay { id } => overlay(&cache, id)?,
        Command::Object { id } => object(&cache, id)?,
        Command::FindObject { query, limit } => find_object(&cache, &query, limit)?,
        Command::Model { id } => model(&cache, id)?,
        Command::ExportObj { id, out } => export_obj(&cache, id, &out)?,
        Command::TerrainObj {
            region_x,
            region_y,
            out,
        } => terrain_obj(&cache, region_x, region_y, &out)?,
    }

    Ok(())
}

fn cache_info(cache: &Cache) -> Result<()> {
    println!("cache: {}", cache.dir().display());
    println!(
        "keys: {}",
        cache
            .keys()
            .map(|k| format!("{} entries", k.len()))
            .unwrap_or_else(|| "none".to_string())
    );

    for index in 0..=255u32 {
        let count = cache.group_count(index);
        if count > 0 {
            let protocol = cache
                .reference(index)
                .ok()
                .map(|r| format!("protocol {}", r.protocol))
                .unwrap_or_else(|| "no reference".to_string());
            println!("index {index:3}: {count:6} groups  {protocol}");
        }
    }
    Ok(())
}

fn map_groups(cache: &Cache, region_x: u32, region_y: u32) -> Result<(u32, u32)> {
    let keys = cache
        .keys()
        .context("cache has no keys.json; cannot resolve map group names")?;
    let locations = keys
        .find_locations_group(region_x, region_y)
        .with_context(|| format!("no locations group for region {region_x},{region_y}"))?;
    if locations == 0 {
        bail!("locations group 0 has no terrain predecessor");
    }
    Ok((locations - 1, locations))
}

fn terrain(cache: &Cache, region_x: u32, region_y: u32) -> Result<()> {
    let (terrain_group, _) = map_groups(cache, region_x, region_y)?;
    let group = cache.group(5, terrain_group)?;
    let terrain = Terrain::load(region_x, region_y, &group.data)?;
    let stats = terrain.stats();

    println!(
        "region {region_x},{region_y}: terrain group {terrain_group}, {} bytes, compression {}",
        group.data.len(),
        group.compression
    );
    println!(
        "tiles: {}  overlays: {}  underlays: {}  blocked: {}",
        terrain.tiles().len(),
        stats.overlays,
        stats.underlays,
        stats.blocked
    );

    let mut top = stats
        .heights
        .iter()
        .enumerate()
        .filter(|&(_, &n)| n > 0)
        .collect::<Vec<_>>();
    top.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    println!("top heights: {:?}", top.iter().take(8).collect::<Vec<_>>());

    println!("\nplane 0 height map (64x64, one char per tile):");
    let chars = b" .:-=+*#%@";
    for y in (0..64).rev() {
        let mut line = String::with_capacity(64);
        for x in 0..64 {
            let h = terrain.height(0, x, y) as usize;
            line.push(chars[(h * (chars.len() - 1) / 255).min(chars.len() - 1)] as char);
        }
        println!("{line}");
    }

    Ok(())
}

fn locations(cache: &Cache, region_x: u32, region_y: u32) -> Result<()> {
    let (_, locations_group) = map_groups(cache, region_x, region_y)?;
    let group = cache.group(5, locations_group)?;
    let locations = rift_cache::Locations::load(region_x, region_y, &group.data)?;
    let mut counts = locations.counts_by_id().into_iter().collect::<Vec<_>>();
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

    println!(
        "region {region_x},{region_y}: locations group {locations_group}, {} bytes, {} placements",
        group.data.len(),
        locations.locations.len()
    );
    for (id, count) in counts.iter().take(30) {
        let name = cache
            .config(6, *id)
            .ok()
            .and_then(|data| ObjectDefinition::parse(*id, &data).ok())
            .map(|def| def.name)
            .unwrap_or_default();
        println!("  object {id:6} x{count:<4} {name}");
    }
    Ok(())
}

fn underlay(cache: &Cache, id: u32) -> Result<()> {
    let data = cache.file(2, 1, id)?;
    let def = rift_cache::UnderlayDefinition::parse(id, &data)?;
    println!(
        "underlay {id}: rgb=#{:06x} color={:?}",
        def.rgb,
        def.color()
    );
    Ok(())
}

fn overlay(cache: &Cache, id: u32) -> Result<()> {
    let data = cache.file(2, 4, id)?;
    let def = rift_cache::OverlayDefinition::parse(id, &data)?;
    println!(
        "overlay {id}: rgb=#{:06x} color={:?} texture={} hide_underlay={}",
        def.rgb,
        def.color(),
        def.texture,
        def.hide_underlay
    );
    Ok(())
}

fn object(cache: &Cache, id: u32) -> Result<()> {
    let data = cache.config(6, id)?;
    let def = ObjectDefinition::parse(id, &data)?;
    println!(
        "object {id}: {:?} models={:?} types={:?} size={}x{} anim={}",
        def.name, def.models, def.model_types, def.size_x, def.size_y, def.animation_id
    );
    println!(
        "  wall_or_door={} contoured={} merge_normals={} clipped={} ambient={} contrast={}",
        def.wall_or_door,
        def.contoured_ground,
        def.merge_normals,
        def.model_clipped,
        def.ambient,
        def.contrast
    );
    Ok(())
}

fn find_object(cache: &Cache, query: &str, limit: usize) -> Result<()> {
    let files = cache.archive_files(2, 6)?;
    let query_lower = query.to_lowercase();
    let mut found = 0usize;
    for (id, data) in files.iter().enumerate() {
        if let Ok(def) = ObjectDefinition::parse(id as u32, data) {
            if def.name.to_lowercase().contains(&query_lower) {
                println!(
                    "object {:6}: {:?} models={:?} types={:?} size={}x{}",
                    id, def.name, def.models, def.model_types, def.size_x, def.size_y
                );
                found += 1;
                if found >= limit {
                    break;
                }
            }
        }
    }
    println!("{found} match(es)");
    Ok(())
}

fn model(cache: &Cache, id: u32) -> Result<()> {
    let group = cache.group(7, id)?;
    let model = Model::load(id, &group.data)?;
    let (min, max) = model.bounds();
    println!(
        "model {id}: {} verts, {} faces, {} texture faces, {} bytes",
        model.vertex_count,
        model.face_count,
        model.num_texture_faces,
        group.data.len()
    );
    println!("  bounds min={min:?} max={max:?}");
    println!(
        "  groups={:?} textures={}",
        model.vertex_groups.as_ref().map(Vec::len),
        model.face_textures.is_some()
    );
    Ok(())
}

fn export_obj(cache: &Cache, id: u32, out: &PathBuf) -> Result<()> {
    let group = cache.group(7, id)?;
    let model = Model::load(id, &group.data)?;
    let mut f = std::fs::File::create(out)?;

    writeln!(f, "# rift model {id}")?;
    for i in 0..model.vertex_count {
        writeln!(
            f,
            "v {} {} {}",
            model.vertex_x[i], model.vertex_y[i], model.vertex_z[i]
        )?;
    }
    for face in &model.faces {
        writeln!(f, "f {} {} {}", face[0] + 1, face[1] + 1, face[2] + 1)?;
    }
    println!(
        "wrote {} ({} verts, {} faces)",
        out.display(),
        model.vertex_count,
        model.face_count
    );
    Ok(())
}

fn terrain_obj(cache: &Cache, region_x: u32, region_y: u32, out: &PathBuf) -> Result<()> {
    let (terrain_group, _) = map_groups(cache, region_x, region_y)?;
    let group = cache.group(5, terrain_group)?;
    let terrain = Terrain::load(region_x, region_y, &group.data)?;
    let mut f = std::fs::File::create(out)?;

    writeln!(f, "# rift terrain {region_x},{region_y}")?;
    for y in 0..64 {
        for x in 0..64 {
            let wx = x as i32 * 128;
            let wz = y as i32 * 128;
            let h = terrain.height(0, x, y) as i32 * 8;
            writeln!(f, "v {wx} {h} {wz}")?;
        }
    }
    for y in 0..63 {
        for x in 0..63 {
            let a = y * 64 + x + 1;
            let b = a + 1;
            let c = a + 64;
            let d = c + 1;
            writeln!(f, "f {a} {c} {b}")?;
            writeln!(f, "f {b} {c} {d}")?;
        }
    }
    println!("wrote {} ({} tiles)", out.display(), terrain.tiles().len());
    Ok(())
}
