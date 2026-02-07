use gps::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE_PATH: &str = "test_data/test_map.map";

    #[test]
    fn test_read_header() -> Result<()> {
        let file = File::open(TEST_FILE_PATH)?;
        let mut reader = BufReader::new(file);

        let header = MapHeader::read_from_file(&mut reader)?;

        assert_eq!(header.magic, "mapsforge binary OSM");
        assert_eq!(header.tile_size, 256);
        assert!(header.is_valid());

        Ok(())
    }

    #[test]
    fn test_valid_bounding_box() {
        let test_data = vec![
            0x00, 0x00, 0x00, 0x0A, // min_lat: 10/1_000_000 = 0.00001
            0x00, 0x00, 0x00, 0x14, // min_lon: 20/1_000_000 = 0.00002
            0x00, 0x00, 0x00, 0x28, // max_lat: 40/1_000_000 = 0.00004
            0x00, 0x00, 0x00, 0x32, // max_lon: 50/1_000_000 = 0.00005
        ];

        let mut reader = BufReader::new(Cursor::new(test_data));
        let bbox = BoundingBox::read_from_buffer(&mut reader).unwrap();

        assert_eq!(bbox.min_lat, 0.00001);
        assert_eq!(bbox.min_lon, 0.00002);
        assert_eq!(bbox.max_lat, 0.00004);
        assert_eq!(bbox.max_lon, 0.00005);
    }

    #[test]
    fn test_invalid_bounding_box() {
        let test_data = vec![
            0x00, 0x00, 0x00, 0x5A, // min_lat: 90 degrees
            0x00, 0x00, 0x00, 0x14, // min_lon: 20 degrees
            0x00, 0x00, 0x00, 0x28, // max_lat: 40 degrees (invalid because < min_lat)
            0x00, 0x00, 0x00, 0x32, // max_lon: 50 degrees
        ];

        let mut reader = BufReader::new(Cursor::new(test_data));
        let result = BoundingBox::read_from_buffer(&mut reader);

        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }
}
