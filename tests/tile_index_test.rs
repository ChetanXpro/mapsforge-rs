use std::{
    io::{BufReader, Cursor},
    path::PathBuf,
};

use gps::{types::MapFile, MapHeader, MapforgeError, Result};

#[test]
fn test_tile_index_reading() -> Result<()> {
    let test_file_path = PathBuf::from("test_data/test_map.map");
    let map_file = MapFile::open(test_file_path)?;

    assert_eq!(
        map_file.tile_indices.len(),
        map_file.header.num_zoom_intervals as usize
    );

    if let Some(first_interval) = map_file.tile_indices.first() {
        let expected_tiles = MapFile::calculate_total_tiles(
            &map_file.header.bounding_box,
            map_file.header.zoom_interval_configuration[0].base_zoom_level,
        );
        assert_eq!(first_interval.len(), expected_tiles as usize);

        if let Some(first_tile) = first_interval.first() {
            assert!(first_tile.offset > 0 || first_tile.is_water);
        }
    }

    Ok(())
}

#[test]
fn test_vbe_signed_int() {
    // Single byte positive
    let data = vec![0x05]; // +5
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_s_int(&mut reader).unwrap(), 5);

    // Single byte negative
    let data = vec![0x45]; // -5 (0x40 is sign bit)
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_s_int(&mut reader).unwrap(), -5);

    // Multi-byte value
    let data = vec![0x80, 0x01]; // 128
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_u_int(&mut reader).unwrap(), 128);
}

#[test]
fn test_tile_out_of_bounds() -> Result<()> {
    let test_file_path = PathBuf::from("test_data/test_map.map");
    let mut map_file = MapFile::open(test_file_path)?;

    // Coordinates outside map bounds but valid zoom
    // Use a zoom level that exists in your test map
    let valid_zoom = map_file.header.zoom_interval_configuration[0].base_zoom_level;
    let result = map_file.get_tile_at(0.0, 0.0, valid_zoom);

    assert!(matches!(
        result,
        Err(MapforgeError::TileOutOfBounds) | Err(MapforgeError::ZoomLevelNotSupported)
    ));

    Ok(())
}

#[test]
fn test_unsupported_zoom_level() -> Result<()> {
    let test_file_path = PathBuf::from("test_data/test_map.map");
    let mut map_file = MapFile::open(test_file_path)?;

    // Zoom level 25 is unlikely to be supported
    let result = map_file.get_tile_at(51.5, -0.1, 25);

    assert!(matches!(result, Err(MapforgeError::ZoomLevelNotSupported)));

    Ok(())
}

#[test]
fn test_zoom_config() -> Result<()> {
    let map_file = MapFile::open("test_data/central-zone.map")?;

    println!("=== ZOOM INTERVALS ===");
    for (i, interval) in map_file
        .header
        .zoom_interval_configuration
        .iter()
        .enumerate()
    {
        println!(
            "Interval {}: base={}, min={}, max={}",
            i, interval.base_zoom_level, interval.min_zoom_level, interval.max_zoom_level
        );
    }

    // Test the index calculation
    let test_zoom: u8 = 5;
    for (i, interval) in map_file
        .header
        .zoom_interval_configuration
        .iter()
        .enumerate()
    {
        if test_zoom >= interval.min_zoom_level && test_zoom <= interval.max_zoom_level {
            let current_zoom_index = (test_zoom - interval.min_zoom_level) as usize;
            println!(
                "\nZoom {} falls in interval {}: min={}, so current_zoom_index = {} - {} = {}",
                test_zoom,
                i,
                interval.min_zoom_level,
                test_zoom,
                interval.min_zoom_level,
                current_zoom_index
            );
        }
    }

    Ok(())
}

#[test]
fn test_zoom_zero() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;

    // Request zoom 0, which should give current_zoom_index = 0
    let tile = map_file.get_tile_at(lat, lon, 0)?;

    println!("Zoom table: {:?}", tile.zoom_table);
    println!("POIs parsed: {}", tile.pois.len());
    println!("Ways parsed: {}", tile.ways.len());

    // Should now see 228 ways!
    for (i, way) in tile.ways.iter().take(5).enumerate() {
        let tags: Vec<&String> = way
            .tag_ids
            .iter()
            .filter_map(|&id| map_file.header.way_tags.get(id as usize))
            .collect();

        let coord_count: usize = way
            .coordinate_blocks
            .iter()
            .map(|b| b.coordinates.len())
            .sum();

        println!(
            "Way {}: tags={:?}, name={:?}, coords={}",
            i, tags, way.name, coord_count
        );
    }

    Ok(())
}

#[test]
fn test_zoom_five_cumulative() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;

    let tile = map_file.get_tile_at(lat, lon, 5)?;

    println!("Zoom table: {:?}", tile.zoom_table);
    println!("POIs parsed: {}", tile.pois.len());
    println!("Ways parsed: {}", tile.ways.len());

    // At zoom 5, we should get cumulative from index 0-5
    // Index 0: 228 ways
    // Index 1-5: 0 + 0 + 0 + 1 + 0 = 1 way
    // Total expected: 229 ways

    Ok(())
}

#[test]
fn test_zoom_seven() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;

    let tile = map_file.get_tile_at(lat, lon, 7)?;

    println!("Zoom table: {:?}", tile.zoom_table);
    println!("POIs parsed: {}", tile.pois.len());
    println!("Ways parsed: {}", tile.ways.len());

    // At zoom 7, cumulative from index 0-7
    // Should include index 6 which has 42 POIs, 15918 ways
    // Expected: 43 POIs, ~16147 ways

    // Show some different tag types
    let mut tag_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for way in &tile.ways {
        for &tag_id in &way.tag_ids {
            if let Some(tag) = map_file.header.way_tags.get(tag_id as usize) {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }
        }
    }

    println!("\n=== WAY TAGS SUMMARY ===");
    let mut sorted: Vec<_> = tag_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (tag, count) in sorted.iter().take(15) {
        println!("  {}: {}", tag, count);
    }

    Ok(())
}

#[test]
fn test_zoom_levels_availability() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;

    println!("=== TESTING ALL ZOOM LEVELS ===\n");

    for zoom in 0..=21 {
        print!("Zoom {:2}: ", zoom);

        match map_file.get_tile_at(lat, lon, zoom) {
            Ok(tile) => {
                println!("✓ POIs={:5}, Ways={:6}", tile.pois.len(), tile.ways.len());
            }
            Err(e) => {
                println!("✗ {:?}", e);
            }
        }
    }

    Ok(())
}

#[test]
fn test_verify_poi_on_map() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    let (tile_lat, tile_lon) = map_file
        .get_tile_origin(lat, lon, zoom)
        .expect("Should get tile origin");

    println!("Tile origin: lat={:.6}, lon={:.6}\n", tile_lat, tile_lon);
    println!("=== POIs with OpenStreetMap links ===\n");

    for poi in &tile.pois {
        let abs_lat = tile_lat + (poi.position_offset.0 as f64 / 1_000_000.0);
        let abs_lon = tile_lon + (poi.position_offset.1 as f64 / 1_000_000.0);

        let display_name = poi
            .name
            .as_ref()
            .map(|n| n.split('\r').next().unwrap_or(n))
            .unwrap_or("unnamed");

        // ✅ Determine appropriate zoom based on POI type
        let osm_zoom = if poi.tag.iter().any(|t| t.contains("place=country")) {
            5 // Country level
        } else if poi.tag.iter().any(|t| t.contains("place=state")) {
            7 // State/province level
        } else if poi.tag.iter().any(|t| t.contains("place=city")) {
            11 // City level
        } else if poi.tag.iter().any(|t| t.contains("place=town")) {
            13 // Town level
        } else if poi.tag.iter().any(|t| t.contains("place=village")) {
            14 // Village level
        } else if poi
            .tag
            .iter()
            .any(|t| t.contains("place=suburb") || t.contains("place=neighbourhood"))
        {
            15 // Neighborhood level
        } else if poi.tag.iter().any(|t| t.contains("amenity=")) {
            16 // Building/POI level (restaurants, banks, etc.)
        } else {
            12 // Default for unknown types
        };

        println!("{}", display_name);
        println!("  Tags: {:?}", poi.tag);
        println!("  Lat: {:.6}, Lon: {:.6}", abs_lat, abs_lon);
        println!(
            "  OSM: https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}&zoom={}",
            abs_lat, abs_lon, osm_zoom
        );
        println!();
    }

    Ok(())
}

#[test]
fn test_coordinate_accuracy() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    // Known real coordinates (from OpenStreetMap):
    // Haridwar: 29.9457° N, 78.1642° E
    // Delhi/India Gate: 28.6129° N, 77.2295° E
    // Agra (Taj Mahal): 27.1751° N, 78.0421° E

    println!("=== MAP FILE BOUNDING BOX ===");
    println!("min_lat: {}", map_file.header.bounding_box.min_lat);
    println!("max_lat: {}", map_file.header.bounding_box.max_lat);
    println!("min_lon: {}", map_file.header.bounding_box.min_lon);
    println!("max_lon: {}", map_file.header.bounding_box.max_lon);

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let n = 2_f64.powi(zoom as i32);
    let tile_x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let tile_y = ((1.0
        - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
        / 2.0
        * n)
        .floor() as u32;

    println!("\n=== TILE CALCULATION ===");
    println!("Request: lat={}, lon={}, zoom={}", lat, lon, zoom);
    println!("Tile X: {}, Tile Y: {}", tile_x, tile_y);

    // Calculate tile bounds (not just corner)
    let tile_lon_min = (tile_x as f64 / n) * 360.0 - 180.0;
    let tile_lon_max = ((tile_x + 1) as f64 / n) * 360.0 - 180.0;

    let tile_lat_max_rad = std::f64::consts::PI * (1.0 - 2.0 * tile_y as f64 / n);
    let tile_lat_min_rad = std::f64::consts::PI * (1.0 - 2.0 * (tile_y + 1) as f64 / n);
    let tile_lat_max = tile_lat_max_rad.sinh().atan().to_degrees();
    let tile_lat_min = tile_lat_min_rad.sinh().atan().to_degrees();

    println!("\nTile bounds:");
    println!("  Lat: {:.6} to {:.6}", tile_lat_min, tile_lat_max);
    println!("  Lon: {:.6} to {:.6}", tile_lon_min, tile_lon_max);

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    // Find Haridwar in the POIs
    println!("\n=== LOOKING FOR KNOWN CITIES ===");
    for poi in &tile.pois {
        let name = poi
            .name
            .as_ref()
            .map(|n| n.split('\r').next().unwrap_or(n))
            .unwrap_or("");

        if name.contains("Haridw")
            || name.contains("Agra")
            || name.contains("Delhi")
            || name.contains("Noida")
        {
            let abs_lat = tile_lat_max + (poi.position_offset.0 as f64 / 1_000_000.0);
            let abs_lon = tile_lon_min + (poi.position_offset.1 as f64 / 1_000_000.0);

            println!("\n{}", name);
            println!(
                "  Offset: lat={}, lon={}",
                poi.position_offset.0, poi.position_offset.1
            );
            println!("  Calculated: lat={:.6}, lon={:.6}", abs_lat, abs_lon);

            // Expected coordinates (rough)
            let expected = match name {
                n if n.contains("Haridw") => (29.9457, 78.1642),
                n if n.contains("Agra") => (27.1751, 78.0421),
                n if n.contains("Noida") => (28.5355, 77.3910),
                _ => (0.0, 0.0),
            };

            if expected.0 > 0.0 {
                println!("  Expected:   lat={:.6}, lon={:.6}", expected.0, expected.1);
                println!(
                    "  Difference: lat={:.6}, lon={:.6}",
                    abs_lat - expected.0,
                    abs_lon - expected.1
                );
            }
        }
    }

    Ok(())
}

#[test]
fn test_coordinate_fix() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let n = 2_f64.powi(zoom as i32);
    let tile_x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let tile_y = ((1.0
        - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
        / 2.0
        * n)
        .floor() as u32;

    // Tile corners
    let tile_lon_min = (tile_x as f64 / n) * 360.0 - 180.0; // 73.125
    let tile_lon_max = ((tile_x + 1) as f64 / n) * 360.0 - 180.0; // 78.75

    let tile_lat_max_rad = std::f64::consts::PI * (1.0 - 2.0 * tile_y as f64 / n);
    let tile_lat_min_rad = std::f64::consts::PI * (1.0 - 2.0 * (tile_y + 1) as f64 / n);
    let tile_lat_max = tile_lat_max_rad.sinh().atan().to_degrees(); // 31.95
    let tile_lat_min = tile_lat_min_rad.sinh().atan().to_degrees(); // 27.06

    println!("Tile bounds:");
    println!(
        "  Lat: {:.6} (min) to {:.6} (max)",
        tile_lat_min, tile_lat_max
    );
    println!(
        "  Lon: {:.6} (min) to {:.6} (max)",
        tile_lon_min, tile_lon_max
    );

    println!("\n=== TESTING DIFFERENT INTERPRETATIONS ===\n");

    // Test with Haridwar: expected 29.9457, 78.1642
    // Offset: lat=-2013715, lon=10645298

    let test_cases = vec![
        ("Haridwār", -2013715_i32, 10645298_i32, 29.9457, 78.1642),
        ("Agra", -4776907, 10509816, 27.1751, 78.0421),
        ("Noida", -3381529, 9827214, 28.5355, 77.3910),
    ];

    for (name, lat_off, lon_off, expected_lat, expected_lon) in test_cases {
        println!("{}:", name);
        println!("  Offset: lat={}, lon={}", lat_off, lon_off);

        // Method 1: Current (tile_lat_max + offset, tile_lon_min + offset)
        let m1_lat = tile_lat_max + (lat_off as f64 / 1_000_000.0);
        let m1_lon = tile_lon_min + (lon_off as f64 / 1_000_000.0);
        println!(
            "  Method 1 (max_lat + off, min_lon + off): {:.4}, {:.4} | diff: {:.4}, {:.4}",
            m1_lat,
            m1_lon,
            m1_lat - expected_lat,
            m1_lon - expected_lon
        );

        // Method 2: tile_lat_max + offset, tile_lon_max - offset
        let m2_lat = tile_lat_max + (lat_off as f64 / 1_000_000.0);
        let m2_lon = tile_lon_max - (lon_off as f64 / 1_000_000.0);
        println!(
            "  Method 2 (max_lat + off, max_lon - off): {:.4}, {:.4} | diff: {:.4}, {:.4}",
            m2_lat,
            m2_lon,
            m2_lat - expected_lat,
            m2_lon - expected_lon
        );

        // Method 3: Both from min
        let m3_lat = tile_lat_min + (lat_off as f64 / 1_000_000.0);
        let m3_lon = tile_lon_min + (lon_off as f64 / 1_000_000.0);
        println!(
            "  Method 3 (min_lat + off, min_lon + off): {:.4}, {:.4} | diff: {:.4}, {:.4}",
            m3_lat,
            m3_lon,
            m3_lat - expected_lat,
            m3_lon - expected_lon
        );

        // Method 4: Use map bounding box instead of tile
        let m4_lat = map_file.header.bounding_box.max_lat + (lat_off as f64 / 1_000_000.0);
        let m4_lon = map_file.header.bounding_box.min_lon + (lon_off as f64 / 1_000_000.0);
        println!("  Method 4 (bbox max_lat + off, bbox min_lon + off): {:.4}, {:.4} | diff: {:.4}, {:.4}", 
            m4_lat, m4_lon, m4_lat - expected_lat, m4_lon - expected_lon);

        // Method 5: Subtract tile width from lon
        let m5_lat = tile_lat_max + (lat_off as f64 / 1_000_000.0);
        let m5_lon = tile_lon_min + (lon_off as f64 / 1_000_000.0) - 5.625;
        println!(
            "  Method 5 (current - tile_width): {:.4}, {:.4} | diff: {:.4}, {:.4}",
            m5_lat,
            m5_lon,
            m5_lat - expected_lat,
            m5_lon - expected_lon
        );

        println!();
    }

    Ok(())
}

#[test]
fn test_base_zoom_tile_corner() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let requested_zoom: u8 = 6;

    // The base zoom for this interval is 5, not 6!
    let base_zoom: u8 = 5;

    let n_base = 2_f64.powi(base_zoom as i32);
    let tile_x_base = ((lon + 180.0) / 360.0 * n_base).floor() as u32;
    let tile_y_base = ((1.0
        - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
        / 2.0
        * n_base)
        .floor() as u32;

    // Base zoom tile corners
    let tile_lon_min_base = (tile_x_base as f64 / n_base) * 360.0 - 180.0;

    let tile_lat_max_rad_base = std::f64::consts::PI * (1.0 - 2.0 * tile_y_base as f64 / n_base);
    let tile_lat_max_base = tile_lat_max_rad_base.sinh().atan().to_degrees();

    println!("Requested zoom: {}", requested_zoom);
    println!("Base zoom: {}", base_zoom);
    println!("Base tile X: {}, Y: {}", tile_x_base, tile_y_base);
    println!(
        "Base tile corner: lat={:.6}, lon={:.6}",
        tile_lat_max_base, tile_lon_min_base
    );

    let tile = map_file.get_tile_at(lat, lon, requested_zoom)?;

    println!("\n=== TESTING WITH BASE ZOOM CORNER ===\n");

    let test_cases = vec![
        ("Haridwār", -2013715_i32, 10645298_i32, 29.9457, 78.1642),
        ("Agra", -4776907_i32, 10509816_i32, 27.1751, 78.0421),
        ("Noida", -3381529_i32, 9827214_i32, 28.5355, 77.3910),
    ];

    for (name, lat_off, lon_off, expected_lat, expected_lon) in test_cases {
        let calc_lat = tile_lat_max_base + (lat_off as f64 / 1_000_000.0);
        let calc_lon = tile_lon_min_base + (lon_off as f64 / 1_000_000.0);

        println!("{}:", name);
        println!("  Calculated: {:.4}, {:.4}", calc_lat, calc_lon);
        println!("  Expected:   {:.4}, {:.4}", expected_lat, expected_lon);
        println!(
            "  Difference: {:.4}, {:.4}",
            calc_lat - expected_lat,
            calc_lon - expected_lon
        );
        println!();
    }

    Ok(())
}

#[test]
fn test_exact_coordinates() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    // Get the base zoom from interval config
    let interval = &map_file.header.zoom_interval_configuration[0];
    let base_zoom = interval.base_zoom_level;

    println!("Base zoom level: {}", base_zoom);

    let n = 2_f64.powi(base_zoom as i32);
    let tile_x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let tile_y = ((1.0
        - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
        / 2.0
        * n)
        .floor() as u32;

    println!(
        "Tile X: {}, Tile Y: {} (at base zoom {})",
        tile_x, tile_y, base_zoom
    );

    // Calculate tile corner with maximum precision
    let tile_lon_min = (tile_x as f64 / n) * 360.0 - 180.0;
    let tile_lat_max_rad = std::f64::consts::PI * (1.0 - 2.0 * tile_y as f64 / n);
    let tile_lat_max = tile_lat_max_rad.sinh().atan().to_degrees();

    println!("Tile top-left corner:");
    println!("  lat_max (top):  {:.10}", tile_lat_max);
    println!("  lon_min (left): {:.10}", tile_lon_min);

    // According to spec, coordinates are stored in MICRODEGREES
    // So offset / 1_000_000 gives degrees

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    // Find Haridwar
    for poi in &tile.pois {
        let name = poi
            .name
            .as_ref()
            .map(|n| n.split('\r').next().unwrap_or(n))
            .unwrap_or("");

        if name.contains("Haridw") {
            println!("\n=== {} ===", name);
            println!("Stored offset (microdegrees):");
            println!("  lat_diff: {}", poi.position_offset.0);
            println!("  lon_diff: {}", poi.position_offset.1);

            // The spec says the offset is from TOP-LEFT corner
            // Top = max lat, Left = min lon
            let calc_lat = tile_lat_max + (poi.position_offset.0 as f64 / 1_000_000.0);
            let calc_lon = tile_lon_min + (poi.position_offset.1 as f64 / 1_000_000.0);

            println!("\nCalculated position:");
            println!("  lat: {:.10}", calc_lat);
            println!("  lon: {:.10}", calc_lon);

            // Real Haridwar coordinates from OSM:
            // https://www.openstreetmap.org/node/245158183
            // Actually let's check what OSM has
            println!("\nOSM link to verify:");
            println!(
                "  https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}&zoom=14",
                calc_lat, calc_lon
            );
        }
    }

    Ok(())
}
