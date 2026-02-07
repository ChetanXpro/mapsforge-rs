use std::{
    f64::consts::PI,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use byteorder::{BigEndian, ReadBytesExt};

use crate::{
    error::MapforgeError,
    header::DEBUG_INFO_MASK,
    types::{BoundingBox, MapFile, MapHeader, Tile, TileIndexEntry, Way, WayCoordinateBlock, POI},
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
      
        let zoom_level_index = self
            .header
            .zoom_interval_configuration
            .iter()
            .position(|interval| zoom >= interval.min_zoom_level && zoom <= interval.max_zoom_level)
            .ok_or(MapforgeError::ZoomLevelNotSupported)?;

        let zoom_interval_start =
            self.header.zoom_interval_configuration[zoom_level_index].sub_file_start;
        let min_zoom = self.header.zoom_interval_configuration[zoom_level_index].min_zoom_level;
        let max_zoom = self.header.zoom_interval_configuration[zoom_level_index].max_zoom_level;

        let tile_index_entry = self.calculate_tile_entry(lat, lon, zoom)?;
        let tile_offset = tile_index_entry.offset;

        let absolute_offset = zoom_interval_start + tile_offset;

        self.reader.seek(SeekFrom::Start(absolute_offset))?;

        // TILE HEADER - Debug signature
        if self.header.flags & DEBUG_INFO_MASK != 0 {
            let mut bytes = [0u8; 32];
            self.reader.read_exact(&mut bytes)?;
            let sig = String::from_utf8_lossy(&bytes);
            println!("DEBUG: tile signature = '{}'", sig.trim());

            if !sig.starts_with("###TileStart") {
                return Err(MapforgeError::InvalidTileSignature);
            }
        }

        // Zoom table
        let num_zoom_levels = max_zoom - min_zoom + 1;

        let mut zoom_table: Vec<(u32, u32)> = Vec::with_capacity(num_zoom_levels as usize);
        for _ in 0..num_zoom_levels {
            let poi_count = MapHeader::read_vbe_u_int(&mut self.reader)?;
            let way_count = MapHeader::read_vbe_u_int(&mut self.reader)?;
            zoom_table.push((poi_count, way_count));
        }

        println!("DEBUG: zoom_table = {:?}", zoom_table);

        // Save position BEFORE reading first_way_offset
        let first_way_offset_position = self.reader.stream_position()?;

        // First way offset (VBE-U INT)
        let first_way_offset = MapHeader::read_vbe_u_int(&mut self.reader)?;
        println!("DEBUG: first_way_offset = {}", first_way_offset);
        println!(
            "DEBUG: first_way_offset_position = {}",
            first_way_offset_position
        );

        // Position right after first_way_offset is read
        let after_first_way_offset = self.reader.stream_position()?;

        // Calculate where ways actually start
        // first_way_offset is counted from the byte AFTER the first_way_offset field
        let ways_absolute_position = after_first_way_offset + first_way_offset as u64;
        println!("DEBUG: ways_absolute_position = {}", ways_absolute_position);

        let current_zoom_index = (zoom - min_zoom) as usize;
        let poi_count = zoom_table[current_zoom_index].0;

        println!("DEBUG: current_zoom_index = {}", current_zoom_index);
        println!("DEBUG: poi_count for this zoom = {}", poi_count);

        if poi_count > 50000 {
            println!("ERROR: POI count {} is suspiciously high", poi_count);
            return Err(MapforgeError::InvalidTileData);
        }

        let poi_tags_len = self.header.poi_tags.len();
        let mut poi_data: Vec<POI> = Vec::with_capacity(poi_count as usize);

        // POI READING (we're already at the right position - right after first_way_offset)
        for i in 0..poi_count {
            if self.header.flags & DEBUG_INFO_MASK != 0 {
                let mut bytes = [0u8; 32];
                self.reader.read_exact(&mut bytes)?;
                let sig = String::from_utf8_lossy(&bytes);

                if !sig.starts_with("***POIStart") {
                    println!(
                        "ERROR: Invalid POI signature at index {}: '{}'",
                        i,
                        sig.trim()
                    );
                    return Err(MapforgeError::InvalidTilePOISignature);
                }
            }

            let lat_diff = MapHeader::read_vbe_s_int(&mut self.reader)?;
            let lon_diff = MapHeader::read_vbe_s_int(&mut self.reader)?;

            let special_byte = self.reader.read_u8()?;
            let layer = (((special_byte & 0xf0) >> 4) as i8) - 5;
            let num_tags = (special_byte & 0x0f) as u32;

            let mut tags: Vec<String> = Vec::with_capacity(num_tags as usize);
            for _ in 0..num_tags {
                let tag_id = MapHeader::read_vbe_u_int(&mut self.reader)?;

                if (tag_id as usize) < poi_tags_len {
                    let tag_name = &self.header.poi_tags[tag_id as usize];

                    // Check if this tag has a wildcard value (version 5+)
                    if self.header.file_version >= 5 && self.header.tag_has_wildcard(tag_name) {
                        // Read the variable value
                        let value = MapHeader::read_vbe_u(&mut self.reader)?;
                        // Replace * with actual value: "name=*" → "name=Restaurant"
                        let tag_with_value = tag_name.replace('*', &value);
                        tags.push(tag_with_value);
                    } else {
                        // No wildcard, just use tag name
                        tags.push(tag_name.clone());
                    }
                } else if self.header.file_version >= 5 {
                    // Invalid tag ID in v5 - might still have a value to skip
                    // This is defensive: skip the value if it exists
                    // We can't know for sure, so this might cause issues
                    // Better to validate tag_id is valid
                    return Err(MapforgeError::InvalidTagId);
                }
            }

            let flags = self.reader.read_u8()?;
            let has_name = (flags & 0x80) != 0;
            let has_house_number = (flags & 0x40) != 0;
            let has_elevation = (flags & 0x20) != 0;

            let name = if has_name {
                Some(MapHeader::read_vbe_u(&mut self.reader)?)
            } else {
                None
            };

            let house_number = if has_house_number {
                Some(MapHeader::read_vbe_u(&mut self.reader)?)
            } else {
                None
            };

            let elevation = if has_elevation {
                Some(MapHeader::read_vbe_s_int(&mut self.reader)?)
            } else {
                None
            };

            if i < 5 {
                println!(
                    "DEBUG POI {}: lat_diff={}, lon_diff={}, layer={}, tags={:?}, name={:?}",
                    i, lat_diff, lon_diff, layer, tags, name
                );
            }

            poi_data.push(POI {
                position_offset: (lat_diff, lon_diff),
                layer,
                tag: tags,
                elevation,
                house_number,
                name,
            });
        }

        println!("DEBUG: Successfully parsed {} POIs", poi_data.len());

        // Now seek to where ways actually start
        println!(
            "DEBUG: Current position after POIs = {}",
            self.reader.stream_position()?
        );
        println!(
            "DEBUG: Seeking to ways_absolute_position = {}",
            ways_absolute_position
        );
        self.reader.seek(SeekFrom::Start(ways_absolute_position))?;

        // Parse ways
        let way_count = zoom_table[current_zoom_index].1;

        let ways = self.parse_ways_count(way_count)?;

        Ok(Tile {
            first_way_offset,
            zoom_table,
            pois: poi_data,
            ways,
        })
    }

    pub fn parse_ways_count(&mut self, way_count: u32) -> Result<Vec<Way>> {
        println!("DEBUG: Parsing {} ways", way_count);

        if way_count > 100000 {
            return Err(MapforgeError::InvalidTileData);
        }

        let mut ways: Vec<Way> = Vec::with_capacity(way_count as usize);

        for i in 0..way_count {
            // Debug signature (32 bytes if debug flag set)
            if self.header.flags & DEBUG_INFO_MASK != 0 {
                let mut bytes = [0u8; 32];
                self.reader.read_exact(&mut bytes)?;
                let sig = String::from_utf8_lossy(&bytes);

                if !sig.starts_with("---WayStart") {
                    println!(
                        "ERROR: Invalid Way signature at index {}: '{}'",
                        i,
                        sig.trim()
                    );
                    return Err(MapforgeError::InvalidWaySignature);
                }
            }

            // Way data size (VBE-U INT) - IMPORTANT: we can use this to skip if parsing fails
            let way_data_size = MapHeader::read_vbe_u_int(&mut self.reader)?;
            let way_start_pos = self.reader.stream_position()?;

            // Sub tile bitmap (2 bytes)
            let sub_tile_bitmap = self.reader.read_u16::<BigEndian>()?;

            // Special byte
            let special_byte = self.reader.read_u8()?;
            let layer = (((special_byte & 0xf0) >> 4) as i8) - 5;
            let num_tags = (special_byte & 0x0f) as u32;

            // Tag IDs
            // Tag IDs
            let mut tag_ids: Vec<u32> = Vec::with_capacity(num_tags as usize);
            for _ in 0..num_tags {
                let tag_id = MapHeader::read_vbe_u_int(&mut self.reader)?;

                // Validate tag ID is within bounds
                if tag_id as usize >= self.header.way_tags.len() {
                    return Err(MapforgeError::InvalidTagId);
                }

                tag_ids.push(tag_id);

                // For version 5+, check if tag has wildcard and read value
                if self.header.file_version >= 5 {
                    let tag_name = &self.header.way_tags[tag_id as usize];
                    if self.header.tag_has_wildcard(tag_name) {
                        // Read and discard the value
                        let _value = MapHeader::read_vbe_u(&mut self.reader)?;
                    }
                }
            }

            // Flags byte
            let flags = self.reader.read_u8()?;
            let has_name = (flags & 0x80) != 0;
            let has_house_number = (flags & 0x40) != 0;
            let has_ref = (flags & 0x20) != 0;
            let has_label_position = (flags & 0x10) != 0;
            let has_num_way_blocks = (flags & 0x08) != 0;
            let double_delta_encoding = (flags & 0x04) != 0;

            // Optional fields
            let name = if has_name {
                Some(MapHeader::read_vbe_u(&mut self.reader)?)
            } else {
                None
            };

            let house_number = if has_house_number {
                Some(MapHeader::read_vbe_u(&mut self.reader)?)
            } else {
                None
            };

            let reference = if has_ref {
                Some(MapHeader::read_vbe_u(&mut self.reader)?)
            } else {
                None
            };

            let label_position = if has_label_position {
                let lat_diff = MapHeader::read_vbe_s_int(&mut self.reader)?;
                let lon_diff = MapHeader::read_vbe_s_int(&mut self.reader)?;
                Some((lat_diff, lon_diff))
            } else {
                None
            };

            // Number of way data blocks (only if flag is set, otherwise 1)
            let num_way_data_blocks = if has_num_way_blocks {
                MapHeader::read_vbe_u_int(&mut self.reader)?
            } else {
                1
            };

            // Parse coordinate blocks
            let mut coordinate_blocks: Vec<WayCoordinateBlock> = Vec::new();

            // For each way data block
            for _data_block in 0..num_way_data_blocks {
                // Read number of coordinate blocks for this data block
                let num_coord_blocks = MapHeader::read_vbe_u_int(&mut self.reader)?;

                if num_coord_blocks == 0 || num_coord_blocks > 100 {
                    println!(
                        "WARNING Way {}: suspicious num_coord_blocks = {}",
                        i, num_coord_blocks
                    );
                }

                for _coord_block in 0..num_coord_blocks {
                    let num_nodes = MapHeader::read_vbe_u_int(&mut self.reader)?;

                    if num_nodes == 0 {
                        continue;
                    }

                    if num_nodes > 10000 {
                        println!("  ERROR: num_nodes {} is suspiciously large!", num_nodes);
                        return Err(MapforgeError::InvalidTileData);
                    }

                    let first_lat = MapHeader::read_vbe_s_int(&mut self.reader)?;
                    let first_lon = MapHeader::read_vbe_s_int(&mut self.reader)?;

                    let mut coordinates: Vec<(i32, i32)> = Vec::with_capacity(num_nodes as usize);
                    coordinates.push((first_lat, first_lon));

                    if double_delta_encoding {
                        // Double delta decoding
                        let mut previous_lat = first_lat;
                        let mut previous_lon = first_lon;
                        let mut previous_lat_offset = 0i32;
                        let mut previous_lon_offset = 0i32;
                        let mut count = 0;

                        for _ in 1..num_nodes {
                            let lat_encoded = MapHeader::read_vbe_s_int(&mut self.reader)?;
                            let lon_encoded = MapHeader::read_vbe_s_int(&mut self.reader)?;

                            let current_lat = previous_lat
                                .saturating_add(previous_lat_offset)
                                .saturating_add(lat_encoded);
                            let current_lon = previous_lon
                                .saturating_add(previous_lon_offset)
                                .saturating_add(lon_encoded);

                            if count > 0 {
                                previous_lat_offset = current_lat.saturating_sub(previous_lat);
                                previous_lon_offset = current_lon.saturating_sub(previous_lon);
                            }

                            previous_lat = current_lat;
                            previous_lon = current_lon;

                            coordinates.push((current_lat, current_lon));
                            count += 1;
                        }
                    } else {
                        // Single delta decoding
                        let mut current_lat = first_lat;
                        let mut current_lon = first_lon;

                        for _ in 1..num_nodes {
                            let lat_delta = MapHeader::read_vbe_s_int(&mut self.reader)?;
                            let lon_delta = MapHeader::read_vbe_s_int(&mut self.reader)?;

                            current_lat = current_lat.saturating_add(lat_delta);
                            current_lon = current_lon.saturating_add(lon_delta);

                            coordinates.push((current_lat, current_lon));
                        }
                    }

                    coordinate_blocks.push(WayCoordinateBlock {
                        initial_position: (first_lat, first_lon),
                        coordinates,
                    });
                }
            }

            // Verify we read exactly way_data_size bytes
            let way_end_pos = self.reader.stream_position()?;
            let bytes_read = way_end_pos - way_start_pos;

            if bytes_read != way_data_size as u64 {
                // Data size mismatch indicates parser is out of sync - fail hard
                return Err(MapforgeError::InvalidTileData);
            }

            if i < 3 {
                println!(
                    "DEBUG Way {}: layer={}, tags={:?}, name={:?}, blocks={}, size={}",
                    i,
                    layer,
                    tag_ids,
                    name,
                    coordinate_blocks.len(),
                    way_data_size
                );
            }

            ways.push(Way {
                sub_tile_bitmap,
                layer,
                tag_ids,
                name,
                house_number,
                reference,
                label_position,
                coordinate_blocks,
                double_delta_encoding,
            });
        }

        println!("DEBUG: Successfully parsed {} ways", ways.len());

        Ok(ways)
    }

    fn calculate_tile_entry(&mut self, lat: f64, lon: f64, zoom: u8) -> Result<&TileIndexEntry> {
        let zoom_level_index = self
            .header
            .zoom_interval_configuration
            .iter()
            .position(|interval| zoom >= interval.min_zoom_level && zoom <= interval.max_zoom_level)
            .ok_or(MapforgeError::ZoomLevelNotSupported)?;

        let interval = &self.header.zoom_interval_configuration[zoom_level_index];
        let base_zoom = interval.base_zoom_level;

        let tiles_for_zoom = &self.tile_indices[zoom_level_index];

        let (x, y) = Self::get_tile_coordinates(lat, lon, base_zoom);

        let x_min = ((self.header.bounding_box.min_lon + 180.0) / 360.0
            * 2_f64.powi(base_zoom as i32))
        .floor() as i64;
        let x_max = ((self.header.bounding_box.max_lon + 180.0) / 360.0
            * 2_f64.powi(base_zoom as i32))
        .floor() as i64;

        let lat_rad_max = self.header.bounding_box.max_lat.to_radians();
        let y_min = ((1.0 - (lat_rad_max.tan() + 1.0 / lat_rad_max.cos()).ln() / PI) / 2.0
            * 2_f64.powi(base_zoom as i32))
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

    pub fn get_way_tags(&self, way: &Way) -> Vec<String> {
        way.tag_ids
            .iter()
            .filter_map(|&id| self.header.way_tags.get(id as usize).cloned())
            .collect()
    }
}

impl Tile {
    pub fn get_absolute_poi_position(&self, poi: &POI, tile_lat: f64, tile_lon: f64) -> (f64, f64) {
        let lat = tile_lat + (poi.position_offset.0 as f64 / 1_000_000.0);
        let lon = tile_lon + (poi.position_offset.1 as f64 / 1_000_000.0);
        (lat, lon)
    }
}

// In your mapsforge-rs library
impl MapFile {
    pub fn get_tile_origin(&self, lat: f64, lon: f64, zoom: u8) -> Option<(f64, f64)> {
        let interval = self
            .header
            .zoom_interval_configuration
            .iter()
            .find(|i| zoom >= i.min_zoom_level && zoom <= i.max_zoom_level)?;

        let base_zoom = interval.base_zoom_level;
        let n = 2_f64.powi(base_zoom as i32);

        let tile_x = ((lon + 180.0) / 360.0 * n).floor() as u32;
        let tile_y = ((1.0
            - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI)
            / 2.0
            * n)
            .floor() as u32;

        let tile_lon_min = (tile_x as f64 / n) * 360.0 - 180.0;
        let tile_lat_max_rad = std::f64::consts::PI * (1.0 - 2.0 * tile_y as f64 / n);
        let tile_lat_max = tile_lat_max_rad.sinh().atan().to_degrees();

        Some((tile_lat_max, tile_lon_min))
    }
}
