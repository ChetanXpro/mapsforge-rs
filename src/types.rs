use std::{fs::File, io::BufReader};

#[derive(Debug,Clone, Copy)]
pub struct BoundingBox {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

#[derive(Debug)]
pub struct ZoomInterval {
    pub base_zoom_level: u8,
    pub min_zoom_level: u8,
    pub max_zoom_level: u8,
    pub sub_file_start: u64,
    pub sub_file_size: u64
   
}

#[derive(Debug)]
pub struct MapHeader {
    pub magic: String,
    pub header_size: u32,
    pub file_version: u32,
    pub file_size: u64,
    pub creation_date: u64,
    pub bounding_box: BoundingBox,
    pub tile_size: u16,
    pub projection: String,
    pub flags: u8,

    // optional fields
    pub map_start_position: Option<(f64,f64)>,
    pub start_zoom_level: Option<u8>,
    pub language_preference: Option<String>,
    pub comment: Option<String>,
    pub created_by: Option<String>,


    // tag info
    pub poi_tags: Vec<String>,
    pub way_tags: Vec<String>,
    
    // // zoom interval
    pub num_zoom_intervals: u8,
    pub zoom_interval_configuration: Vec<ZoomInterval>
   
}




// #[derive(Debug)]
// pub struct TileIndexHeader {
//     pub debug_signature: Option<String>,
// }

#[derive(Debug,Clone, Copy)]
pub struct TileIndexEntry {
    pub is_water: bool,   
    pub offset: u64,         
}

#[derive(Debug)]
pub struct MapFile {
    pub header: MapHeader,
    pub reader: BufReader<File>,  
    pub tile_indices: Vec<Vec<TileIndexEntry>>,
}

#[derive(Debug)]
pub struct Tile {

    pub zoom_table: Vec<(u32, u32)>, 
    pub first_way_offset: u32,
    
  
    pub pois: Vec<POI>,
    // pub ways: Vec<Way>
}

#[derive(Debug)]
pub struct POI {

  
    
    pub position_offset: (f64, f64),
    pub layer: i8,
    pub tag: Vec<String>,
    pub name: Option<String>,
    pub house_number: Option<String>, 
    pub elevation: Option<i32>
}
#[derive(Debug)]
pub struct Way {

    
    pub sub_tile_bitmap: u16, 
    pub layer: i8, 
    pub tag_ids: Vec<u32>,
    pub name: Option<String>,
    pub house_number: Option<String>,
    pub reference: Option<String>,
    pub label_position: Option<(i32, i32)>,  
    

    pub coordinate_blocks: Vec<WayCoordinateBlock>,
    

    pub double_delta_encoding: bool
}
#[derive(Debug)]
pub struct WayCoordinateBlock {
    
    pub initial_position: (i32, i32), 
   
    pub coordinates: Vec<(i32, i32)>
}

#[derive(Debug)]
pub struct TagMapping {
    pub poi_tags: Vec<String>,
    pub way_tags: Vec<String>
}