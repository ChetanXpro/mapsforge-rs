
use mapsforge_rs::types::MapFile;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut map = MapFile::open("sample/central-zone.map")?;
    
    // println!("Header: {:#?}", map.tile_indices);

    let lat = 28.6139; 
    let lon = 77.2090;
    let zoom = 14;

    let tile = map.get_tile_at(lat, lon, zoom)?;

    println!("{:#?}",tile);
    
    // for (i, indices) in map.tile_indices.iter().enumerate() {
    //     let zoom_interval = &map.header.zoom_interval_configuration[i];
        
    //     println!("\nZoom Interval {}", i);
    //     println!("Base zoom level: {}", zoom_interval.base_zoom_level);
    //     println!("Total tiles: {}", indices.len());
        
    
    //     println!("\nFirst 5 tile entries:");
    //     for (j, tile) in indices.iter().take(5).enumerate() {
    //         println!("Tile {}: is_water: {}, offset: {}", j, tile.is_water, tile.offset);
    //     }
    // }
    
    Ok(())
}