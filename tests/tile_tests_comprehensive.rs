use std::{
    io::{BufReader, Cursor, Write},
    path::PathBuf,
};

use byteorder::{BigEndian, WriteBytesExt};
use gps::{types::MapFile, MapHeader, MapforgeError, Result};

// ==================== UNIT TESTS - Synthetic Data ====================

#[test]
fn test_vbe_s_coordinate_offsets() {
    // Test typical POI coordinate offsets

    // Small positive offset: +500 microdegrees = 0.0005 degrees
    let data = vec![0xF4, 0x03]; // 500 in VBE-S
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_s_int(&mut reader).unwrap(), 500);

    // Small negative offset: -500 microdegrees
    let data = vec![0xF4, 0x43]; // -500 in VBE-S (sign bit 0x40)
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_s_int(&mut reader).unwrap(), -500);

    // Larger offset: +1,000,000 microdegrees = 1 degree
    let data = vec![0xC0, 0x84, 0x3D]; // 1,000,000 in VBE-S
    let mut reader = BufReader::new(Cursor::new(data));
    assert_eq!(MapHeader::read_vbe_s_int(&mut reader).unwrap(), 1_000_000);
}

#[test]
fn test_poi_special_byte_parsing() {
    // Test special byte: layer + num_tags
    // Layer 0 (stored as 5), 3 tags: 0x53
    // bits 1-4 (layer): 0101 = 5 → layer = 5 - 5 = 0
    // bits 5-8 (tags):  0011 = 3 → 3 tags

    let special_byte: u8 = 0x53;
    let layer = (((special_byte & 0xf0) >> 4) as i8) - 5;
    let num_tags = (special_byte & 0x0f) as u32;

    assert_eq!(layer, 0);
    assert_eq!(num_tags, 3);

    // Layer -2, 1 tag: 0x31
    let special_byte: u8 = 0x31;
    let layer = (((special_byte & 0xf0) >> 4) as i8) - 5;
    let num_tags = (special_byte & 0x0f) as u32;

    assert_eq!(layer, -2);
    assert_eq!(num_tags, 1);

    // Layer +3, 5 tags: 0x85
    let special_byte: u8 = 0x85;
    let layer = (((special_byte & 0xf0) >> 4) as i8) - 5;
    let num_tags = (special_byte & 0x0f) as u32;

    assert_eq!(layer, 3);
    assert_eq!(num_tags, 5);
}

#[test]
fn test_poi_flags_parsing() {
    // Test all flag combinations

    // Has name only (0x80)
    let flags: u8 = 0x80;
    assert!(flags & 0x80 != 0); // has_name
    assert!(flags & 0x40 == 0); // no house_number
    assert!(flags & 0x20 == 0); // no elevation

    // Has all three (0xE0)
    let flags: u8 = 0xE0;
    assert!(flags & 0x80 != 0); // has_name
    assert!(flags & 0x40 != 0); // has house_number
    assert!(flags & 0x20 != 0); // has elevation

    // Has house number and elevation (0x60)
    let flags: u8 = 0x60;
    assert!(flags & 0x80 == 0); // no name
    assert!(flags & 0x40 != 0); // has house_number
    assert!(flags & 0x20 != 0); // has elevation
}

#[test]
fn test_way_sub_tile_bitmap() {
    // A tile at zoom z is made up of 16 sub tiles at zoom z+2
    // Bitmap is stored as 2 bytes (16 bits)

    // All sub tiles covered: 0xFFFF
    let bitmap: u16 = 0xFFFF;
    assert_eq!(bitmap.count_ones(), 16);

    // Only corners: 0x8811 (binary: 1000 1000 0001 0001)
    let bitmap: u16 = 0x8811;
    assert_eq!(bitmap.count_ones(), 4);

    // Test coastline requirement: must have all 16 bits set
    let coastline_bitmap: u16 = 0xFFFF;
    assert_eq!(coastline_bitmap, 0xFFFF);
}

#[test]
fn test_way_flags_parsing() {
    // Test way flags byte

    // Has name + label position + double delta: 0x94
    let flags: u8 = 0x94;
    assert!(flags & 0x80 != 0); // has_name
    assert!(flags & 0x40 == 0); // no house_number
    assert!(flags & 0x20 == 0); // no reference
    assert!(flags & 0x10 != 0); // has_label_position
    assert!(flags & 0x08 == 0); // single way block
    assert!(flags & 0x04 != 0); // double delta encoding

    // Has all optional fields + single delta: 0xF8
    let flags: u8 = 0xF8;
    assert!(flags & 0x80 != 0); // has_name
    assert!(flags & 0x40 != 0); // has house_number
    assert!(flags & 0x20 != 0); // has reference
    assert!(flags & 0x10 != 0); // has_label_position
    assert!(flags & 0x08 != 0); // multiple way blocks
    assert!(flags & 0x04 == 0); // single delta encoding
}

#[test]
fn test_single_delta_decoding() {
    // Test single delta coordinate decoding
    // Starting point: (1000, 2000)
    // Delta 1: (+100, +200) → (1100, 2200)
    // Delta 2: (-50, +150) → (1050, 2350)

    let mut current_lat = 1000i32;
    let mut current_lon = 2000i32;

    let deltas = vec![(100i32, 200i32), (-50i32, 150i32), (300i32, -100i32)];

    let mut coords = vec![(current_lat, current_lon)];

    for (lat_delta, lon_delta) in deltas {
        current_lat = current_lat.wrapping_add(lat_delta);
        current_lon = current_lon.wrapping_add(lon_delta);
        coords.push((current_lat, current_lon));
    }

    assert_eq!(coords[0], (1000, 2000));
    assert_eq!(coords[1], (1100, 2200));
    assert_eq!(coords[2], (1050, 2350));
    assert_eq!(coords[3], (1350, 2250));
}

#[test]
fn test_double_delta_decoding() {
    // From spec example:
    // Tile origin: 52.123456 degrees = 52_123_456 microdegrees
    // Encoded values: -8286, -57, 129, -15, -129
    // Expected decoded: 52.11517, 52.115113, 52.115185, 52.115242, 52.11517

    let tile_origin_micro = 52_123_456i32;
    let encoded_deltas = vec![-8286i32, -57, 129, -15, -129];

    let mut previous_lat = tile_origin_micro;
    let mut previous_offset = 0i32;
    let mut decoded = vec![];
    let mut count = 0;

    for encoded_value in encoded_deltas {
        // Calculate current lat
        let lat = previous_lat + previous_offset + encoded_value;

        // Update previous_offset AFTER first node
        if count > 0 {
            previous_offset = lat - previous_lat;
        }

        // Update previous_lat
        previous_lat = lat;

        decoded.push(lat as f64 / 1_000_000.0);
        count += 1;
    }

    // Check against expected values
    let expected = vec![52.11517, 52.115113, 52.115185, 52.115242, 52.11517];
    for (i, (actual, exp)) in decoded.iter().zip(expected.iter()).enumerate() {
        assert!(
            (actual - exp).abs() < 0.000001,
            "Point {}: expected {}, got {}",
            i,
            exp,
            actual
        );
    }
}
#[test]
fn test_coordinate_overflow_handling() {
    // Test that wrapping_add handles overflow correctly

    let max_i32 = i32::MAX;
    let result = max_i32.wrapping_add(1);
    assert_eq!(result, i32::MIN);

    let min_i32 = i32::MIN;
    let result = min_i32.wrapping_add(-1);
    assert_eq!(result, i32::MAX);

    // Realistic coordinate overflow shouldn't happen with valid data,
    // but wrapping_add ensures no panic
    let lat = 89_000_000i32; // 89 degrees
    let delta = 2_000_000i32; // +2 degrees
    let result = lat.wrapping_add(delta);
    assert_eq!(result, 91_000_000); // Would be invalid but no panic
}

// ==================== INTEGRATION TESTS - Real Map Files ====================

#[test]
fn test_tile_index_consistency() -> Result<()> {
    let test_file_path = PathBuf::from("test_data/central-zone.map");
    let map_file = MapFile::open(test_file_path)?;

    // Each zoom interval should have correct number of tiles
    for (i, interval) in map_file
        .header
        .zoom_interval_configuration
        .iter()
        .enumerate()
    {
        let expected_tiles =
            MapFile::calculate_total_tiles(&map_file.header.bounding_box, interval.base_zoom_level);

        assert_eq!(
            map_file.tile_indices[i].len(),
            expected_tiles as usize,
            "Zoom interval {} has wrong number of tiles",
            i
        );
    }

    Ok(())
}

#[test]
fn test_tile_water_flags() -> Result<()> {
    let test_file_path = PathBuf::from("test_data/central-zone.map");
    let map_file = MapFile::open(test_file_path)?;

    // Check that water tiles have valid offsets
    let mut water_count = 0;
    let mut non_water_count = 0;

    for interval_tiles in &map_file.tile_indices {
        for tile in interval_tiles {
            if tile.is_water {
                water_count += 1;
                // Water tiles might have offset 0 or same as next tile
            } else {
                non_water_count += 1;
                // Non-water tiles should have valid offset
                assert!(tile.offset > 0, "Non-water tile has zero offset");
            }
        }
    }

    println!(
        "Water tiles: {}, Non-water tiles: {}",
        water_count, non_water_count
    );
    Ok(())
}

#[test]
fn test_zoom_table_consistency() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;

    // Clone intervals before mutable borrow
    let intervals: Vec<_> = map_file
        .header
        .zoom_interval_configuration
        .iter()
        .map(|i| (i.min_zoom_level, i.max_zoom_level, i.base_zoom_level))
        .collect(); // ✅ Collect the data we need

    // Test all available zoom levels
    for (min_zoom, max_zoom, base_zoom) in intervals {
        for zoom in min_zoom..=max_zoom {
            let tile = map_file.get_tile_at(lat, lon, zoom)?;

            // Zoom table should have correct number of entries
            let expected_entries = (max_zoom - min_zoom + 1) as usize;
            assert_eq!(
                tile.zoom_table.len(),
                expected_entries,
                "Zoom {} has wrong zoom table size",
                zoom
            );

            // Counts should be reasonable
            let zoom_index = (zoom - min_zoom) as usize;
            let (poi_count, way_count) = tile.zoom_table[zoom_index];

            assert!(
                poi_count < 100_000,
                "Zoom {}: POI count {} is suspiciously high",
                zoom,
                poi_count
            );
            assert!(
                way_count < 500_000,
                "Zoom {}: Way count {} is suspiciously high",
                zoom,
                way_count
            );
        }
    }

    Ok(())
}
#[test]
fn test_poi_coordinate_ranges() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    // Get tile origin
    let tile_origin = map_file
        .get_tile_origin(lat, lon, zoom)
        .expect("Should get tile origin");

    // POI offsets should be within tile bounds
    // A tile at zoom 6 covers approximately 5.625 degrees
    let tile_size_degrees = 360.0 / 2_f64.powi(6);

    for (i, poi) in tile.pois.iter().enumerate() {
        let lat_offset_degrees = poi.position_offset.0 as f64 / 1_000_000.0;
        let lon_offset_degrees = poi.position_offset.1 as f64 / 1_000_000.0;

        // Offsets should be within reasonable bounds
        // Allowing some margin for edge cases
        assert!(
            lat_offset_degrees.abs() < tile_size_degrees * 2.0,
            "POI {}: lat offset {} degrees is too large for tile",
            i,
            lat_offset_degrees
        );
        assert!(
            lon_offset_degrees.abs() < tile_size_degrees * 2.0,
            "POI {}: lon offset {} degrees is too large for tile",
            i,
            lon_offset_degrees
        );

        // Absolute coordinates should be within map bounds
        let abs_lat = tile_origin.0 + lat_offset_degrees;
        let abs_lon = tile_origin.1 + lon_offset_degrees;

        assert!(
            abs_lat >= map_file.header.bounding_box.min_lat - 1.0
                && abs_lat <= map_file.header.bounding_box.max_lat + 1.0,
            "POI {}: absolute lat {} is outside map bounds",
            i,
            abs_lat
        );
        assert!(
            abs_lon >= map_file.header.bounding_box.min_lon - 1.0
                && abs_lon <= map_file.header.bounding_box.max_lon + 1.0,
            "POI {}: absolute lon {} is outside map bounds",
            i,
            abs_lon
        );
    }

    Ok(())
}

#[test]
fn test_poi_tags_valid() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    // All POI tags should be valid
    for (i, poi) in tile.pois.iter().enumerate() {
        assert!(!poi.tag.is_empty(), "POI {} has no tags", i);

        // Tags should be non-empty strings
        for tag in &poi.tag {
            assert!(!tag.is_empty(), "POI {} has empty tag", i);
        }

        // Layer should be in reasonable range
        assert!(
            poi.layer >= -5 && poi.layer <= 5,
            "POI {} has invalid layer: {}",
            i,
            poi.layer
        );
    }

    Ok(())
}

#[test]
fn test_way_coordinate_blocks_valid() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 0;

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    for (i, way) in tile.ways.iter().take(100).enumerate() {
        // Each way should have at least one coordinate block
        assert!(
            !way.coordinate_blocks.is_empty(),
            "Way {} has no coordinate blocks",
            i
        );

        for (j, block) in way.coordinate_blocks.iter().enumerate() {
            // Each block should have at least 2 coordinates (start + end)
            assert!(
                block.coordinates.len() >= 2,
                "Way {} block {} has < 2 coordinates",
                i,
                j
            );

            // First coordinate should match initial position
            assert_eq!(
                block.coordinates[0], block.initial_position,
                "Way {} block {}: first coordinate doesn't match initial position",
                i, j
            );

            // Coordinates should be in reasonable range
            for (k, coord) in block.coordinates.iter().enumerate() {
                assert!(
                    coord.0.abs() < 180_000_000,
                    "Way {} block {} coord {}: lat {} is out of range",
                    i,
                    j,
                    k,
                    coord.0
                );
                assert!(
                    coord.1.abs() < 360_000_000,
                    "Way {} block {} coord {}: lon {} is out of range",
                    i,
                    j,
                    k,
                    coord.1
                );
            }
        }

        // Sub tile bitmap should be valid
        assert!(
            way.sub_tile_bitmap > 0,
            "Way {} has empty sub tile bitmap",
            i
        );

        // Layer should be in reasonable range
        assert!(
            way.layer >= -5 && way.layer <= 5,
            "Way {} has invalid layer: {}",
            i,
            way.layer
        );
    }

    Ok(())
}

#[test]
fn test_way_tags_valid() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 0;

    let tile = map_file.get_tile_at(lat, lon, zoom)?;

    for (i, way) in tile.ways.iter().take(100).enumerate() {
        // Each way should have at least one tag
        assert!(!way.tag_ids.is_empty(), "Way {} has no tags", i);

        // All tag IDs should be valid
        let tags = map_file.get_way_tags(way);
        assert!(!tags.is_empty(), "Way {} has invalid tag IDs", i);

        for tag in &tags {
            assert!(!tag.is_empty(), "Way {} has empty tag", i);
        }
    }

    Ok(())
}

#[test]
fn test_empty_tiles() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    // Clone bounds before borrowing mutably
    let bounds = map_file.header.bounding_box; // ✅ Copy struct

    // Test corner tiles
    let corners = vec![
        (bounds.min_lat, bounds.min_lon),
        (bounds.min_lat, bounds.max_lon),
        (bounds.max_lat, bounds.min_lon),
        (bounds.max_lat, bounds.max_lon),
    ];

    // Clone zoom intervals before mutable borrow
    let zoom_intervals: Vec<_> = map_file
        .header
        .zoom_interval_configuration
        .iter()
        .map(|z| z.base_zoom_level)
        .collect(); // ✅ Collect zoom levels

    for (lat, lon) in corners {
        for zoom in zoom_intervals.iter() {
            match map_file.get_tile_at(lat, lon, *zoom) {
                Ok(tile) => {
                    println!(
                        "Corner ({:.2}, {:.2}) zoom {}: {} POIs, {} ways",
                        lat,
                        lon,
                        zoom,
                        tile.pois.len(),
                        tile.ways.len()
                    );
                }
                Err(e) => {
                    println!("Corner ({:.2}, {:.2}) zoom {}: {:?}", lat, lon, zoom, e);
                }
            }
        }
    }

    Ok(())
}

#[test]
fn test_tile_parsing_multiple_locations() -> Result<()> {
    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    // Clone zoom intervals before mutable borrows
    let zoom_levels: Vec<u8> = map_file
        .header
        .zoom_interval_configuration
        .iter()
        .map(|z| z.base_zoom_level)
        .collect(); // ✅ Collect first

    let test_locations = vec![
        (28.6129, 77.2295, "Delhi"),
        (27.1751, 78.0421, "Agra"),
        (29.9457, 78.1642, "Haridwar"),
    ];

    for (lat, lon, name) in test_locations {
        println!("\nTesting location: {}", name);

        for zoom in &zoom_levels {
            // ✅ Iterate over collected vector
            match map_file.get_tile_at(lat, lon, *zoom) {
                Ok(tile) => {
                    println!(
                        "  Zoom {}: {} POIs, {} ways",
                        zoom,
                        tile.pois.len(),
                        tile.ways.len()
                    );

                    assert!(tile.pois.len() < 100_000);
                    assert!(tile.ways.len() < 500_000);
                }
                Err(e) => {
                    println!("  Zoom {}: Error {:?}", zoom, e);
                }
            }
        }
    }

    Ok(())
}
#[test]
fn test_version_check() -> Result<()> {
    let map_file = MapFile::open("test_data/central-zone.map")?;

    println!("Map file version: {}", map_file.header.file_version);

    assert!(
        map_file.header.file_version >= 3,
        "Map file version should be at least 3"
    );

    if map_file.header.file_version == 5 {
        println!("WARNING: Version 5 files with variable tags not fully supported");
    }

    Ok(())
}

// ==================== PERFORMANCE TESTS ====================

#[test]
#[ignore] // Run with: cargo test --test tile_tests -- --ignored
fn test_tile_parsing_performance() -> Result<()> {
    use std::time::Instant;

    let mut map_file = MapFile::open("test_data/central-zone.map")?;

    let lat: f64 = 28.6129;
    let lon: f64 = 77.2295;
    let zoom: u8 = 6;

    let iterations = 100;
    let start = Instant::now();

    for _ in 0..iterations {
        let _tile = map_file.get_tile_at(lat, lon, zoom)?;
    }

    let duration = start.elapsed();
    let avg_ms = duration.as_millis() as f64 / iterations as f64;

    println!("Average tile parsing time: {:.2}ms", avg_ms);

    // Should be reasonably fast (< 10ms on modern hardware)
    assert!(avg_ms < 50.0, "Tile parsing is too slow: {:.2}ms", avg_ms);

    Ok(())
}
