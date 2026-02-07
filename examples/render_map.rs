// examples/render_map.rs - Updated version
use gps::{coordinate::CoordinateConverter, renderer::{MapRenderer, colors}, types::MapFile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let map_path = "test_data/central-zone.map";
    let mut map = MapFile::open(map_path)?;
    
    println!("Map loaded successfully!");
    println!("Bounding box: {:?}\n", map.header.bounding_box);
    
    // Try Bhopal city center (more urban, should have data at higher zooms)
    let center_lat = 23.2599;
    let center_lon = 77.4126;
    
    // Also try a more urban location - Indore
    // let center_lat = 22.7196;
    // let center_lon = 75.8577;
    
    let screen_width = 320;
    let screen_height = 240;
    
    // Try zoom levels that work with your map
    let zoom_levels = vec![11, 12, 13, 14];
    
    for zoom in zoom_levels {
        // println!("\n{'='*60}");
        println!("RENDERING ZOOM LEVEL {} at ({}, {})", zoom, center_lat, center_lon);
        // println!("{'='*60}");
        
        match render_single_zoom(&mut map, center_lat, center_lon, zoom, screen_width, screen_height) {
            Ok(stats) => {
                println!("✓ SUCCESS: {} POIs, {} Ways, {} segments rendered", 
                    stats.pois, stats.ways, stats.segments);
            }
            Err(e) => {
                println!("✗ FAILED: {:?}", e);
            }
        }
    }
    
    // println!("\n{'='*60}");
    println!("Rendering complete! Best results are likely zoom 13-14");
    // println!("{'='*60}");
    
    Ok(())
}

struct RenderStats {
    pois: usize,
    ways: usize,
    segments: usize,
}

fn render_single_zoom(
    map: &mut MapFile,
    lat: f64,
    lon: f64,
    zoom: u8,
    width: u16,
    height: u16,
) -> Result<RenderStats, Box<dyn std::error::Error>> {
    let converter = CoordinateConverter::new(zoom, width, height, lat, lon);
    let mut renderer = MapRenderer::new(width, height);
    renderer.clear(colors::LAND);
    
    // Get tile
    let tile = map.get_tile_at(lat, lon, zoom)?;
    let tile_origin = map.get_tile_origin(lat, lon, zoom)
        .ok_or("Failed to get tile origin")?;
    
    println!("Tile data: {} POIs, {} Ways", tile.pois.len(), tile.ways.len());
    
    // Print some interesting tags from the first few ways
    for (i, way) in tile.ways.iter().take(10).enumerate() {
        let tags = map.get_way_tags(way);
        if !tags.is_empty() && !tags[0].contains("nosea") {
            println!("  Way {}: {:?}", i, tags);
        }
    }
    
    let mut segment_count = 0;
    
    // Render ways
    for way in &tile.ways {
        let tags = map.get_way_tags(way);
        let (color, thickness) = classify_way(&tags);
        
        if thickness == 0 {
            continue;
        }
        
        for block in &way.coordinate_blocks {
            let screen_coords = converter.way_coords_to_screen(
                tile_origin,
                &block.coordinates,
            );
            
            if screen_coords.len() >= 2 {
                renderer.draw_way(&screen_coords, color, thickness);
                segment_count += 1;
            }
        }
    }
    
    // Render POIs
    for poi in tile.pois.iter().take(200) {
        let (poi_lat, poi_lon) = tile.get_absolute_poi_position(poi, tile_origin.0, tile_origin.1);
        
        if let Some((x, y)) = converter.latlon_to_screen(poi_lat, poi_lon) {
            // Color by POI type
            let poi_color = if poi.tag.iter().any(|t| t.contains("amenity")) {
                colors::MAGENTA
            } else if poi.tag.iter().any(|t| t.contains("shop")) {
                colors::CYAN
            } else {
                colors::BLUE
            };
            
            renderer.draw_circle(x, y, 3, poi_color, true);
        }
    }
    
    // GPS indicator
    if let Some((x, y)) = converter.latlon_to_screen(lat, lon) {
        renderer.draw_circle(x, y, 8, colors::RED, true);
        renderer.draw_circle(x, y, 10, colors::WHITE, false);
    }
    
    // Save
    let output_path = format!("rendered_zoom_{}.png", zoom);
    renderer.save_as_png(&output_path)?;
    
    Ok(RenderStats {
        pois: tile.pois.len(),
        ways: tile.ways.len(),
        segments: segment_count,
    })
}

fn classify_way(tags: &[String]) -> (u16, u8) {
    for tag in tags {
        let tag_lower = tag.to_lowercase();
        
        if tag_lower.contains("nosea") {
            return (colors::WHITE, 0);
        }
        
        // Water
        if tag_lower.contains("natural=water") || 
           tag_lower.contains("waterway=") {
            return (colors::WATER, 4);
        }
        
        // Major roads
        if tag_lower.contains("highway=motorway") || 
           tag_lower.contains("highway=trunk") {
            return (colors::ROAD_MAJOR, 6);
        }
        
        if tag_lower.contains("highway=primary") {
            return (colors::ROAD_MAJOR, 5);
        }
        
        if tag_lower.contains("highway=secondary") {
            return (colors::ROAD_MAJOR, 4);
        }
        
        if tag_lower.contains("highway=tertiary") {
            return (colors::rgb(255, 200, 0), 3);
        }
        
        if tag_lower.contains("highway=residential") || 
           tag_lower.contains("highway=unclassified") {
            return (colors::WHITE, 2);
        }
        
        if tag_lower.contains("highway=service") ||
           tag_lower.contains("highway=track") {
            return (colors::rgb(220, 220, 220), 1);
        }
        
        // Natural features
        if tag_lower.contains("natural=wood") || 
           tag_lower.contains("landuse=forest") {
            return (colors::PARK, 1);
        }
        
        if tag_lower.contains("natural=scrub") {
            return (colors::rgb(180, 200, 150), 1);
        }
        
        if tag_lower.contains("landuse=farmland") {
            return (colors::rgb(230, 240, 200), 1);
        }
        
        // Buildings - fill them!
        if tag_lower.contains("building=") {
            return (colors::BUILDING, 1);
        }
        
        // Railways
        if tag_lower.contains("railway=") {
            return (colors::BLACK, 3);
        }
        
        // Administrative boundaries
        if tag_lower.contains("boundary=") {
            return (colors::rgb(200, 200, 200), 1);
        }
    }
    
    (colors::rgb(220, 220, 220), 1)
}