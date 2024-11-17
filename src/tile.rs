use std::{
    f64::consts::PI,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::ReadBytesExt;

use crate::{
    error::MapforgeError,
    header::DEBUG_INFO_MASK,
    types::{BoundingBox, MapFile, MapHeader, Tile, TileIndexEntry, POI},
    Result,
};

const TILE_INDEX_SIGNATURE: &str = "+++IndexStart+++";
const TILE_SIGNATURE: &str = "###TileStartX,Y###";
const POI_SIGNATURE: &str = "***POIStartX***";
const WATER_TILE_MASK: u8 = 0x80;

impl MapFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let header = MapHeader::read_from_file(&mut reader)?;

        let mut zoom_tile_indices = Vec::with_capacity(header.num_zoom_intervals as usize);

        for interval in &header.zoom_interval_configuration {
            reader.seek(SeekFrom::Start(interval.sub_file_start))?;

            if header.flags & DEBUG_INFO_MASK != 0 {
                let mut sig = [0u8; 16];
                reader.read_exact(&mut sig)?;
                let index_sig = String::from_utf8_lossy(&sig).trim().to_string();

                if index_sig != TILE_INDEX_SIGNATURE {
                    return Err(MapforgeError::InvalidTileIndexSignature);
                }
            }

            let total_tiles_index =
                Self::calculate_total_tiles(&header.bounding_box, interval.base_zoom_level);

            let mut tile_index = Vec::with_capacity(total_tiles_index as usize);
            for _ in 0..total_tiles_index {
                let mut bytes = [0u8; 5];

                reader.read_exact(&mut bytes)?;

                let is_water_tile = (bytes[0] & WATER_TILE_MASK) != 0;

                bytes[0] &= !WATER_TILE_MASK;

                let tile_index_entry = TileIndexEntry {
                    is_water: is_water_tile,
                    offset: u64::from_be_bytes([
                        0, 0, 0, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4],
                    ]),
                };
                tile_index.push(tile_index_entry);
            }

            zoom_tile_indices.push(tile_index);
        }

        Ok(Self {
            header,
            reader,
            tile_indices: zoom_tile_indices,
        })
    }

    pub fn get_tile_at(&mut self, lat: f64, lon: f64, zoom: u8) -> Result<Tile> {
        let tile_entry = self.calculate_tile_entry(lat, lon, zoom)?.offset;

        self.reader.seek(SeekFrom::Start(tile_entry))?;

        // TILE HEADER READING
        if self.header.flags & DEBUG_INFO_MASK != 0 {
            let mut bytes = [0u8; 32];
            self.reader.read_exact(&mut bytes)?;

            let sig = String::from_utf8_lossy(&bytes).trim().to_string();

            if sig != TILE_SIGNATURE {
                return Err(MapforgeError::InvalidTileSignature);
            }
        }

        let zoom_level_index = self
            .header
            .zoom_interval_configuration
            .iter()
            .position(|interval| zoom >= interval.min_zoom_level && zoom <= interval.max_zoom_level)
            .unwrap();

        let zoom_interval = &self.header.zoom_interval_configuration[zoom_level_index];
        let num_zoom_levels = zoom_interval.max_zoom_level - zoom_interval.min_zoom_level + 1;

        let mut raw_numbers = Vec::new();

        for _ in 0..(num_zoom_levels * 2) {
            let value = MapHeader::read_vbe_u_int(&mut self.reader)?;

            raw_numbers.push(value);
        }

        let zoom_table: Vec<(u32, u32)> = raw_numbers
            .chunks(2)
            .map(|chunk| (chunk[0], chunk[1]))
            .collect();

        let mut bytes = [0u8; 4];
        self.reader.read_exact(&mut bytes)?;
        let first_way_offset = u32::from_be_bytes(bytes);

        let current_zoom_index = zoom - zoom_interval.min_zoom_level;

        let mut poi_data: Vec<POI> = vec![];

        println!("POI EXIST: {}",zoom_table[current_zoom_index as usize].0);

        // POI READING
        for _ in 0..zoom_table[current_zoom_index as usize].0 {
            if self.header.flags & DEBUG_INFO_MASK != 0 {
                let mut bytes = [0u8; 32];
                self.reader.read_exact(&mut bytes)?;

                let sig = String::from_utf8_lossy(&bytes).trim().to_string();

                if sig != POI_SIGNATURE {
                    return Err(MapforgeError::InvalidTilePOISignature);
                }
            }

            let lat = MapHeader::read_vbe_s_int(&mut self.reader)? as f64 / 1_000_000.0;
            let lon = MapHeader::read_vbe_s_int(&mut self.reader)? as f64 / 1_000_000.0;

            // special byte

            let bytes = self.reader.read_u8()?;

            println!("special: {}",bytes);

           
            let layer = (((bytes & 0xf0) >> 4) as i8) - 5;

            let num_tags = bytes & 0x0f;

            let mut tags: Vec<String> = vec![];

            for _ in 0..num_tags {
                let tag_id = MapHeader::read_vbe_u_int(&mut self.reader)?;

                if tag_id < self.header.poi_tags.len() as u32 {
                    tags.push(self.header.poi_tags[tag_id as usize].clone());
                }
            }

            let flag_bytes = self.reader.read_u8()?;

            let has_name = flag_bytes & 0x01 != 0;

            let has_house_number = flag_bytes & 0x02 != 0;

            let has_elevation = flag_bytes & 0x04 != 0;

            let poi_name: Option<String>;

            if has_name {
                poi_name = None;
            } else {
                poi_name = None
            }

            let house_number: Option<String>;

            if has_house_number {
                house_number = None;
            } else {
                house_number = None
            }

            let elevation: Option<i32>;

            if has_elevation {
                elevation = Some(MapHeader::read_vbe_u_int(&mut self.reader)? as i32);
            } else {
                elevation = None
            }

            poi_data.push(POI {
                position_offset: (lat, lon),
                layer: layer as i8,
                tag: tags,
                elevation,
                house_number,
                name: poi_name,
            });
        }

        Ok(Tile {
            first_way_offset,
            zoom_table,
            pois: poi_data,
        })
    }

    fn calculate_tile_entry(&mut self, lat: f64, lon: f64, zoom: u8) -> Result<&TileIndexEntry> {
        let zoom_level_index = self
            .header
            .zoom_interval_configuration
            .iter()
            .position(|interval| zoom >= interval.min_zoom_level && zoom <= interval.max_zoom_level)
            .unwrap();

        let tiles_for_zoom = &self.tile_indices[zoom_level_index];

        let (x, y) = Self::get_tile_coordinates(lat, lon, zoom);

        let x_min = ((self.header.bounding_box.min_lon + 180.0) / 360.0 * 2_f64.powi(zoom as i32))
            .floor() as i64;
        let x_max = ((self.header.bounding_box.max_lon + 180.0) / 360.0 * 2_f64.powi(zoom as i32))
            .floor() as i64;

        let lat_rad_max = self.header.bounding_box.max_lat.to_radians();
        let y_min = ((1.0 - (lat_rad_max.tan() + 1.0 / lat_rad_max.cos()).ln() / PI) / 2.0
            * 2_f64.powi(zoom as i32))
        .floor() as i64;

        let grid_width = x_max - x_min + 1;

        let relative_x = x - x_min;
        let relative_y = y - y_min;

        let tile_entry_index = relative_y * grid_width + relative_x;

        if tile_entry_index < 0 || tile_entry_index >= tiles_for_zoom.len() as i64 {
            return Err(MapforgeError::TileOutOfBounds);
        }

        Ok(&tiles_for_zoom[tile_entry_index as usize])
    }

    fn get_tile_coordinates(lat: f64, lon: f64, zoom: u8) -> (i64, i64) {
        let x = ((lon + 180.0) / 360.0 * 2_f64.powi(zoom as i32)).floor() as i64;

        let lat_rad = lat.to_radians();
        let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0
            * 2_f64.powi(zoom as i32))
        .floor() as i64;

        (x, y)
    }

    pub fn calculate_total_tiles(bounding_box: &BoundingBox, zoom: u8) -> u32 {
        // X calculation (longitude)
        let x_min =
            ((bounding_box.min_lon + 180.0) / 360.0 * 2_f64.powi(zoom as i32)).floor() as i64;
        let x_max =
            ((bounding_box.max_lon + 180.0) / 360.0 * 2_f64.powi(zoom as i32)).floor() as i64;

        // Y calculation (latitude)
        let lat_rad_min = bounding_box.min_lat.to_radians();
        let lat_rad_max = bounding_box.max_lat.to_radians();

        let y_min = ((1.0 - (lat_rad_max.tan() + 1.0 / lat_rad_max.cos()).ln() / PI) / 2.0
            * 2_f64.powi(zoom as i32))
        .floor() as i64;
        let y_max = ((1.0 - (lat_rad_min.tan() + 1.0 / lat_rad_min.cos()).ln() / PI) / 2.0
            * 2_f64.powi(zoom as i32))
        .floor() as i64;

        let num_x = (x_max - x_min + 1) as u32;
        let num_y = (y_max - y_min + 1) as u32;

        let total = num_x * num_y;

        total
    }
}
