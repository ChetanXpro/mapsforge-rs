use std::{
    io::{BufReader, Cursor},
    path::PathBuf,
};

use mapsforge_rs::{types::MapFile, MapHeader, MapforgeError, Result};

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
fn test_delhi_map() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    println!("=== HEADER ===");
    println!("Bounding box: {:?}", map_file.header.bounding_box);
    println!("Tile size: {}", map_file.header.tile_size);
    println!("Zoom intervals: {}", map_file.header.num_zoom_intervals);
    println!("Debug flag: {}", map_file.header.flags & 0x80 != 0);

    // Print zoom interval details
    println!("\n=== ZOOM INTERVALS ===");
    for (i, interval) in map_file
        .header
        .zoom_interval_configuration
        .iter()
        .enumerate()
    {
        println!(
            "Interval {}: base={}, min={}, max={}, start={}, size={}",
            i,
            interval.base_zoom_level,
            interval.min_zoom_level,
            interval.max_zoom_level,
            interval.sub_file_start,
            interval.sub_file_size
        );
    }

    println!("\n=== TILE AT INDIA GATE (trying different zooms) ===");

    // Try different zoom levels
    for zoom in [8, 10, 12, 14, 16] {
        println!("\n--- Zoom {} ---", zoom);
        match map_file.get_tile_at(28.6129, 77.2295, zoom) {
            Ok(tile) => {
                let total_pois: u32 = tile.zoom_table.iter().map(|(p, _)| p).sum();
                let total_ways: u32 = tile.zoom_table.iter().map(|(_, w)| w).sum();
                println!("zoom_table: {:?}", tile.zoom_table);
                println!("Total POIs across all zooms: {}", total_pois);
                println!("Total Ways across all zooms: {}", total_ways);
                println!("Parsed POIs: {}", tile.pois.len());
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }

    Ok(())
}

#[test]
fn test_verify_poi_locations() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 10;

    // Calculate tile corner (top-left)
    let n = 2_f64.powi(zoom as i32);
    let tile_x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let tile_y = ((1.0
        - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
        / 2.0
        * n)
        .floor() as u32;

    // Tile corner coordinates (top-left of tile)
    let tile_lon = (tile_x as f64 / n) * 360.0 - 180.0;
    let tile_lat_rad = std::f64::consts::PI * (1.0 - 2.0 * tile_y as f64 / n);
    let tile_lat = tile_lat_rad.sinh().atan().to_degrees();

    println!("Tile X: {}, Y: {}", tile_x, tile_y);
    println!("Tile corner (top-left): lat={}, lon={}", tile_lat, tile_lon);

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    println!("\n=== VERIFYING POIs ({} total) ===", tile.pois.len());
    for poi in tile.pois.iter().take(10) {
        let poi_lat = tile_lat + (poi.position_offset.0 / 1_000_000.0);
        let poi_lon = tile_lon + (poi.position_offset.1 / 1_000_000.0);

        println!(
            "POI: {:?}\n  Tags: {:?}\n  Position: {:.6}, {:.6}\n  https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}&zoom=16\n",
            poi.name,
            poi.tag,
            poi_lat,
            poi_lon,
            poi_lat,
            poi_lon
        );
    }

    println!("\n=== VERIFYING WAYs ({} total) ===", tile.ways.len());
    for (i, way) in tile.ways.iter().take(5).enumerate() {
        let tags: Vec<&String> = way
            .tag_ids
            .iter()
            .filter_map(|&id| map_file.header.way_tags.get(id as usize))
            .collect();

        if let Some(block) = way.coordinate_blocks.first() {
            if let Some(&(first_lat, first_lon)) = block.coordinates.first() {
                let way_lat = tile_lat + (first_lat as f64 / 1_000_000.0);
                let way_lon = tile_lon + (first_lon as f64 / 1_000_000.0);

                println!(
                    "Way {}: {:?}\n  Tags: {:?}\n  First coord: {:.6}, {:.6}\n  https://www.openstreetmap.org/?mlat={:.6}&mlon={:.6}&zoom=16\n",
                    i,
                    way.name,
                    tags,
                    way_lat,
                    way_lon,
                    way_lat,
                    way_lon
                );
            }
        }
    }

    Ok(())
}
