use crate::{
    error::MapforgeError,
    types::{BoundingBox, MapHeader, ZoomInterval},
    Result,
};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{BufReader, Read};

// masgic bytes that identify a valid Mapsforge binary map file
const MAGIC_BYTES: &str = "mapsforge binary OSM";

const MIN_SUPPORTED_VERSION: u32 = 3;

// mask
pub const DEBUG_INFO_MASK: u8 = 0x80;
const MAP_START_POSITION_MASK: u8 = 0x40;
const START_ZOOM_LEVEL_MASK: u8 = 0x20;
const LANGUAGE_PREFERENCE_MASK: u8 = 0x10;
const COMMENT_MASK: u8 = 0x08;
const CREATED_BY_MASK: u8 = 0x04;

impl BoundingBox {
    // reads a bounding box from a binary buffer
    // using generic so that it can work with any type that implements Read
    pub fn read_from_buffer<R: Read>(reader: &mut BufReader<R>) -> Result<Self> {
        // read and convert coordinates from microdegrees to degrees
        // divide by 1_000_000 to get degrees as all latitude and longitude coordinates are stored in microdegrees (degrees × 10^6)
        let min_lat = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;
        let min_lon = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;
        let max_lat = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;
        let max_lon = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;

        let bbox = BoundingBox {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        };

        if !bbox.is_valid() {
            return Err(MapforgeError::InvalidBoundingBox);
        }

        Ok(bbox)
    }

    // validate if bounding box coordinates are within valid ranges:
    // lat must be between -90 and 90 degree
    // log must be between -180 and 180 degree
    // minimum values must be less then or equal to maximum values
    fn is_valid(&self) -> bool {
        self.min_lat >= -90.0
            && self.min_lat <= 90.0
            && self.max_lat >= -90.0
            && self.max_lat <= 90.0
            && self.min_lon >= -180.0
            && self.min_lon <= 180.0
            && self.max_lon >= -180.0
            && self.max_lon <= 180.0
            && self.min_lat <= self.max_lat
            && self.min_lon <= self.max_lon
    }
}

impl MapHeader {
    // reads a map header from binary buffer
    pub fn read_from_file<R: Read>(reader: &mut BufReader<R>) -> Result<Self> {
        let mut magic_buf = [0u8; 20];
        reader.read_exact(&mut magic_buf)?;
        let magic = String::from_utf8_lossy(&magic_buf).trim().to_string();

        if magic != MAGIC_BYTES {
            return Err(MapforgeError::InvalidMagic);
        }

        let header_size = reader.read_u32::<BigEndian>()?;
        let file_version = reader.read_u32::<BigEndian>()?;

        if file_version < MIN_SUPPORTED_VERSION {
            return Err(MapforgeError::UnsupportedVersion(file_version));
        }

        let file_size = reader.read_u64::<BigEndian>()?;
        let creation_date = reader.read_u64::<BigEndian>()?;

        let bounding_box = BoundingBox::read_from_buffer(reader)?;

        let tile_size = reader.read_u16::<BigEndian>()?;

        let projection = Self::read_vbe_u(reader)?;

        let flags = reader.read_u8()?;

        if flags & DEBUG_INFO_MASK != 0 {
            println!("DEBUG INFO EXIST");
        }

        let map_start_position: Option<(f64, f64)>;

        if flags & MAP_START_POSITION_MASK != 0 {
            let lat = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;
            let lon = reader.read_i32::<BigEndian>()? as f64 / 1_000_000.0;
            map_start_position = Some((lat, lon));
        }else{ 
            map_start_position = None
        }

        let start_zoom_level: Option<u8>;

        if flags & START_ZOOM_LEVEL_MASK != 0 {
            start_zoom_level = Some(reader.read_u8()?);
        } else {
            start_zoom_level = None
        }

        let language_preference: Option<String>;

        if flags & LANGUAGE_PREFERENCE_MASK != 0 {
            language_preference = Some(Self::read_vbe_u(reader)?);
        } else {
            language_preference = None;
        }

        let comment: Option<String>;
        if flags & COMMENT_MASK != 0 {
            comment = Some(Self::read_vbe_u(reader)?)
        }else {
            comment = None
        }

        let created_by: Option<String>;
        if flags & CREATED_BY_MASK != 0 {
            created_by = Some(Self::read_vbe_u(reader)?);
        }else{
            created_by = None
        }


        let num_poi_tags = reader.read_u16::<BigEndian>()?;
        let mut poi_tags: Vec<String> = vec![];

        for _ in 0..num_poi_tags {
            let tag = Self::read_vbe_u(reader)?;
            poi_tags.push(tag);
        }

        let num_way_tags = reader.read_u16::<BigEndian>()?;
        let mut way_tags: Vec<String> = vec![];

        for _ in 0..num_way_tags {
            let tag = Self::read_vbe_u(reader)?;
            way_tags.push(tag);
        }


        let num_zoom_intervals = reader.read_u8()?;
        let mut zoom_interval_configuration:Vec<ZoomInterval> = vec![];


        for _ in 0..num_zoom_intervals {
            let base_zoom_level = reader.read_u8()?;
            let min_zoom_level = reader.read_u8()?;
            let max_zoom_level = reader.read_u8()?;
            let sub_file_start = reader.read_u64::<BigEndian>()?;
            let sub_file_size = reader.read_u64::<BigEndian>()?;


            

            zoom_interval_configuration.push(ZoomInterval{
                base_zoom_level,
                max_zoom_level,
                min_zoom_level,
                sub_file_size,
                sub_file_start
            });


        }


        let header = MapHeader {
            magic,
            header_size,
            file_version,
            file_size,
            creation_date,
            bounding_box,
            tile_size,
            projection,
            flags,
            map_start_position,
            start_zoom_level,
            language_preference,
            comment,
            created_by,
            poi_tags,
            way_tags,
            num_zoom_intervals,
            zoom_interval_configuration

        };

        if !header.is_valid() {
            return Err(MapforgeError::InvalidHeaderSize(header_size));
        }

        Ok(header)
    }

    pub(crate) fn read_vbe_u<R: Read>(reader: &mut BufReader<R>) -> Result<String> {
        let mut length = 0u32;

        let mut shift = 0;

        loop {
            let byte = reader.read_u8()?;

            length |= ((byte & 0x7F) as u32) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7
        }

        let mut string_bytes = vec![0u8; length as usize];

        reader.read_exact(&mut string_bytes)?;

        Ok(String::from_utf8(string_bytes).expect("Error parsing vbe_u"))
    }

    pub(crate) fn read_vbe_u_int<R: Read>(reader: &mut BufReader<R>) -> Result<u32> {

        let mut value = 0u32;

        let mut shift = 0;

        loop {
            let byte = reader.read_u8()?;

            value |= ((byte & 0x7F) as u32) << shift;

            if byte & 0x80 == 0 {
                break;
            }

            shift += 7
        }
        

        Ok(value)

     }

     pub fn read_vbe_s_int<R: Read>(reader: &mut BufReader<R>) -> Result<i32> {
        let mut value = 0i32;
        let mut shift = 0;
    
        // Read bytes until we find one with continuation bit = 0
        loop {
            let byte = reader.read_u8()?;
            
            // Last byte
            if byte & 0x80 == 0 {
                // Last byte uses 6 bits for data and 1 bit for sign
                let is_negative = (byte & 0x40) != 0;
                value |= ((byte & 0x3f) as i32) << shift;
                
                if is_negative {
                    value = -value;
                }
                break;
            }
    
            // Use 7 bits from continuation bytes
            value |= ((byte & 0x7f) as i32) << shift;
            shift += 7;
        }
    
        Ok(value)
    }
    pub fn is_valid(&self) -> bool {
        self.magic.trim() == MAGIC_BYTES
            && self.header_size > 0
            && self.file_version >= MIN_SUPPORTED_VERSION
    }
}
