use gps::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE_PATH: &str = "test_data/test_map.map";

    // ==================== VBE-U INT Tests ====================

    #[test]
    fn test_vbe_u_single_byte() -> Result<()> {
        // Value: 127 (0x7F) - fits in 7 bits
        let data = vec![0x7F];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 127);
        Ok(())
    }

    #[test]
    fn test_vbe_u_zero() -> Result<()> {
        // Value: 0
        let data = vec![0x00];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 0);
        Ok(())
    }

    #[test]
    fn test_vbe_u_two_bytes() -> Result<()> {
        // Value: 300
        // First byte: 0xAC (10101100) - continuation bit set, 7 data bits = 0101100 (44)
        // Second byte: 0x02 (00000010) - continuation bit clear, 7 data bits = 0000010 (2)
        // Result: (2 << 7) | 44 = 256 + 44 = 300
        let data = vec![0xAC, 0x02];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 300);
        Ok(())
    }

    #[test]
    fn test_vbe_u_three_bytes() -> Result<()> {
        // Value: 16384 (2^14)
        // First byte: 0x80 (10000000) - continuation, 7 bits = 0000000
        // Second byte: 0x80 (10000000) - continuation, 7 bits = 0000000
        // Third byte: 0x01 (00000001) - no continuation, 7 bits = 0000001
        // Result: (1 << 14) = 16384
        let data = vec![0x80, 0x80, 0x01];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 16384);
        Ok(())
    }

    #[test]
    fn test_vbe_u_large_value() -> Result<()> {
        // Test a larger value that requires multiple bytes
        // Value: 100000
        let data = vec![0xA0, 0x8D, 0x06];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 100000);
        Ok(())
    }

    #[test]
    fn test_vbe_u_max_safe_value() -> Result<()> {
        // Test maximum value that fits in u32
        // 5 bytes max for VBE-U encoding
        let data = vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_vbe_u_int(&mut reader)?;
        assert!(result > 0);
        Ok(())
    }

    // ==================== VBE-S INT Tests ====================

    #[test]
    fn test_vbe_s_zero() -> Result<()> {
        // Value: 0
        let data = vec![0x00];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, 0);
        Ok(())
    }

    #[test]
    fn test_vbe_s_positive_single_byte() -> Result<()> {
        // Value: +10
        // Byte: 0x0A (00001010) - no continuation, no sign bit, value = 10
        // In VBE-S: bit 0 (MSB) = continuation (0), bits 1-6 = value, bit 7 = unused for last byte
        let data = vec![0x0A];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, 10);
        Ok(())
    }

    #[test]
    fn test_vbe_s_negative_single_byte() -> Result<()> {
        // Value: -10
        // Byte: 0x4A (01001010) - no continuation, sign bit set (0x40), value = 10
        let data = vec![0x4A];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, -10);
        Ok(())
    }

    #[test]
    fn test_vbe_s_positive_max_single_byte() -> Result<()> {
        // Value: +63 (max positive value in single byte)
        // Byte: 0x3F (00111111) - no continuation, no sign bit, 6 bits all set = 63
        let data = vec![0x3F];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, 63);
        Ok(())
    }

    #[test]
    fn test_vbe_s_negative_max_single_byte() -> Result<()> {
        // Value: -63 (max negative value in single byte)
        // Byte: 0x7F (01111111) - no continuation, sign bit set, value = 63
        let data = vec![0x7F];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, -63);
        Ok(())
    }

    #[test]
    fn test_vbe_s_multi_byte_positive() -> Result<()> {
        // Value: +300
        // First byte: 0xAC (10101100) - continuation, 7 bits = 0101100 (44)
        // Second byte: 0x02 (00000010) - no continuation, no sign, 6 bits = 000010 (2)
        // Result: (2 << 7) | 44 = 256 + 44 = 300
        let data = vec![0xAC, 0x02];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, 300);
        Ok(())
    }

    #[test]
    fn test_vbe_s_multi_byte_negative() -> Result<()> {
        // Value: -300
        // First byte: 0xAC (10101100) - continuation, 7 bits = 0101100 (44)
        // Second byte: 0x42 (01000010) - no continuation, sign bit set, 6 bits = 000010 (2)
        // Result: -((2 << 7) | 44) = -(256 + 44) = -300
        let data = vec![0xAC, 0x42];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, -300);
        Ok(())
    }

    #[test]
    fn test_vbe_s_large_positive() -> Result<()> {
        // Value: +100000
        let data = vec![0xA0, 0x8D, 0x06];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, 100000);
        Ok(())
    }

    #[test]
    fn test_vbe_s_large_negative() -> Result<()> {
        // Value: -100000
        let data = vec![0xA0, 0x8D, 0x46];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_s_int(&mut reader)?, -100000);
        Ok(())
    }

    // ==================== VBE-U String Tests ====================

    #[test]
    fn test_vbe_u_string_empty() -> Result<()> {
        // Empty string: length = 0
        let data = vec![0x00];
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, "");
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_simple() -> Result<()> {
        // "hello" = 5 bytes
        let mut data = vec![0x05]; // Length = 5
        data.extend_from_slice(b"hello");
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, "hello");
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_with_spaces() -> Result<()> {
        // "hello world" = 11 bytes
        let mut data = vec![0x0B]; // Length = 11
        data.extend_from_slice(b"hello world");
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, "hello world");
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_utf8() -> Result<()> {
        // "世界" in UTF-8 = 6 bytes
        let utf8_bytes = "世界".as_bytes();
        let mut data = vec![utf8_bytes.len() as u8];
        data.extend_from_slice(utf8_bytes);
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, "世界");
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_utf8_mixed() -> Result<()> {
        // "Hello 世界!" - mix of ASCII and UTF-8
        let text = "Hello 世界!";
        let utf8_bytes = text.as_bytes();
        let mut data = vec![utf8_bytes.len() as u8];
        data.extend_from_slice(utf8_bytes);
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, text);
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_long() -> Result<()> {
        // Test string longer than 127 bytes (requires multi-byte length encoding)
        let text = "a".repeat(200);
        let utf8_bytes = text.as_bytes();
        // Length 200 in VBE-U: 0xC8, 0x01 (200 = 72 + 128*1)
        let mut data = vec![0xC8, 0x01];
        data.extend_from_slice(utf8_bytes);
        let mut reader = BufReader::new(Cursor::new(data));
        assert_eq!(MapHeader::read_vbe_u(&mut reader)?, text);
        Ok(())
    }

    #[test]
    fn test_vbe_u_string_invalid_utf8() {
        // Invalid UTF-8 sequence
        let data = vec![0x03, 0xFF, 0xFE, 0xFD];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_vbe_u(&mut reader);
        // Should return error, not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_vbe_u_string_truncated() {
        // Length says 10 bytes but only 5 provided
        let mut data = vec![0x0A]; // Length = 10
        data.extend_from_slice(b"hello"); // Only 5 bytes
        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_vbe_u(&mut reader);
        // Should error on EOF
        assert!(result.is_err());
    }

    // ==================== Bounding Box Tests ====================

    #[test]
    fn test_bbox_valid_small_area() -> Result<()> {
        // Small valid area around equator/prime meridian
        let data = vec![
            0x00, 0x00, 0x00, 0x0A, // min_lat: 10 microdegrees = 0.00001 degrees
            0x00, 0x00, 0x00, 0x14, // min_lon: 20 microdegrees = 0.00002 degrees
            0x00, 0x00, 0x00, 0x28, // max_lat: 40 microdegrees = 0.00004 degrees
            0x00, 0x00, 0x00, 0x32, // max_lon: 50 microdegrees = 0.00005 degrees
        ];

        let mut reader = BufReader::new(Cursor::new(data));
        let bbox = BoundingBox::read_from_buffer(&mut reader)?;

        assert_eq!(bbox.min_lat, 0.00001);
        assert_eq!(bbox.min_lon, 0.00002);
        assert_eq!(bbox.max_lat, 0.00004);
        assert_eq!(bbox.max_lon, 0.00005);
        Ok(())
    }

    #[test]
    fn test_bbox_valid_world_bounds() -> Result<()> {
        // Full world bounds: -90 to 90 lat, -180 to 180 lon
        // Coordinates stored in microdegrees (degrees × 10^6) as per spec
        // 90_000_000 as i32 Big Endian = 0x055D4A80
        // -90_000_000 as i32 Big Endian = 0xFAA2B580
        // 180_000_000 as i32 Big Endian = 0x0ABA9500
        // -180_000_000 as i32 Big Endian = 0xF5456B00
        let data = vec![
            0xFA, 0xA2, 0xB5, 0x80, // min_lat: -90_000_000 microdegrees = -90 degrees
            0xF5, 0x45, 0x6B, 0x00, // min_lon: -180_000_000 microdegrees = -180 degrees
            0x05, 0x5D, 0x4A, 0x80, // max_lat: 90_000_000 microdegrees = 90 degrees
            0x0A, 0xBA, 0x95, 0x00, // max_lon: 180_000_000 microdegrees = 180 degrees
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let bbox = BoundingBox::read_from_buffer(&mut reader)?;

        assert!((bbox.min_lat - (-90.0)).abs() < 0.000001);
        assert!((bbox.min_lon - (-180.0)).abs() < 0.000001);
        assert!((bbox.max_lat - 90.0).abs() < 0.000001);
        assert!((bbox.max_lon - 180.0).abs() < 0.000001);
        Ok(())
    }

    #[test]
    fn test_bbox_valid_hemisphere() -> Result<()> {
        // Northern hemisphere, eastern hemisphere
        // 90_000_000 as i32 Big Endian = 0x055D4A80
        // 180_000_000 as i32 Big Endian = 0x0ABA9500
        let data = vec![
            0x00, 0x00, 0x00, 0x00, // min_lat: 0 degrees
            0x00, 0x00, 0x00, 0x00, // min_lon: 0 degrees
            0x05, 0x5D, 0x4A, 0x80, // max_lat: 90_000_000 = 90 degrees
            0x0A, 0xBA, 0x95, 0x00, // max_lon: 180_000_000 = 180 degrees
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let bbox = BoundingBox::read_from_buffer(&mut reader)?;

        assert_eq!(bbox.min_lat, 0.0);
        assert_eq!(bbox.min_lon, 0.0);
        assert!((bbox.max_lat - 90.0).abs() < 0.000001);
        assert!((bbox.max_lon - 180.0).abs() < 0.000001);
        Ok(())
    }

    #[test]
    fn test_bbox_point_location() -> Result<()> {
        // Single point (min == max)
        let data = vec![
            0x02, 0xFA, 0xF0, 0x80, // 50.0 degrees
            0x01, 0x31, 0x2D, 0x00, // 20.0 degrees
            0x02, 0xFA, 0xF0, 0x80, // 50.0 degrees (same)
            0x01, 0x31, 0x2D, 0x00, // 20.0 degrees (same)
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let bbox = BoundingBox::read_from_buffer(&mut reader)?;

        assert_eq!(bbox.min_lat, bbox.max_lat);
        assert_eq!(bbox.min_lon, bbox.max_lon);
        Ok(())
    }

    #[test]
    fn test_bbox_invalid_lat_too_high() {
        // Latitude > 90 degrees
        // 91_000_000 as i32 Big Endian = 0x056C5380
        let data = vec![
            0x05, 0x6C, 0x53, 0x80, // 91_000_000 microdegrees = 91 degrees (INVALID)
            0x00, 0x00, 0x00, 0x00, 0x05, 0x5D, 0x4A, 0x80, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    #[test]
    fn test_bbox_invalid_lat_too_low() {
        // Latitude < -90 degrees
        // -91_000_000 as i32 Big Endian = 0xFA93AC80
        let data = vec![
            0xFA, 0x93, 0xAC, 0x80, // -91_000_000 microdegrees = -91 degrees (INVALID)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    #[test]
    fn test_bbox_invalid_lon_too_high() {
        // Longitude > 180 degrees
        // 181_000_000 as i32 Big Endian = 0x0AC99E80
        let data = vec![
            0x00, 0x00, 0x00, 0x00, 0x0A, 0xC9, 0x9E,
            0x80, // 181_000_000 microdegrees = 181 degrees (INVALID)
            0x05, 0x5D, 0x4A, 0x80, 0x0A, 0xBA, 0x95, 0x00,
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    #[test]
    fn test_bbox_invalid_lon_too_low() {
        // Longitude < -180 degrees
        // -181_000_000 as i32 Big Endian = 0xF5366180
        let data = vec![
            0x00, 0x00, 0x00, 0x00, 0xF5, 0x36, 0x61,
            0x80, // -181_000_000 microdegrees = -181 degrees (INVALID)
            0x05, 0x5D, 0x4A, 0x80, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    #[test]
    fn test_bbox_invalid_min_lat_greater_than_max() {
        // min_lat > max_lat
        let data = vec![
            0x05, 0x5D, 0x4A, 0x80, // 90 degrees (min)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 0 degrees (max) - INVALID
            0x00, 0x00, 0x00, 0x00,
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    #[test]
    fn test_bbox_invalid_min_lon_greater_than_max() {
        // min_lon > max_lon
        let data = vec![
            0x00, 0x00, 0x00, 0x00, 0x0A, 0xBA, 0x95, 0x00, // 180 degrees (min)
            0x05, 0x5D, 0x4A, 0x80, 0x00, 0x00, 0x00, 0x00, // 0 degrees (max) - INVALID
        ];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = BoundingBox::read_from_buffer(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidBoundingBox)));
    }

    // ==================== Header Tests ====================

    #[test]
    fn test_header_invalid_magic_bytes() {
        let mut data = vec![0u8; 20];
        data[..15].copy_from_slice(b"invalid magic!!");
        // Add minimal header data to prevent early EOF
        data.extend_from_slice(&[0, 0, 0, 100]); // header_size
        data.extend_from_slice(&[0, 0, 0, 3]); // version

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidMagic)));
    }

    #[test]
    fn test_header_wrong_magic_text() {
        let mut data = b"openstre binary OSM ".to_vec(); // Wrong text, correct length
        data.extend_from_slice(&[0, 0, 0, 100]); // header_size

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(matches!(result, Err(MapforgeError::InvalidMagic)));
    }

    #[test]
    fn test_header_unsupported_version_2() {
        let mut data = b"mapsforge binary OSM".to_vec();
        data.extend_from_slice(&[0, 0, 0, 100]); // header_size
        data.extend_from_slice(&[0, 0, 0, 2]); // version 2 (unsupported)

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(matches!(result, Err(MapforgeError::UnsupportedVersion(2))));
    }

    #[test]
    fn test_header_unsupported_version_1() {
        let mut data = b"mapsforge binary OSM".to_vec();
        data.extend_from_slice(&[0, 0, 0, 100]); // header_size
        data.extend_from_slice(&[0, 0, 0, 1]); // version 1 (unsupported)

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(matches!(result, Err(MapforgeError::UnsupportedVersion(1))));
    }

    #[test]
    fn test_header_truncated_data() {
        let data = b"mapsforge binary OSM".to_vec();
        // Missing header_size and other fields

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        // Should error due to unexpected EOF
        assert!(result.is_err());
    }

    #[test]
    fn test_header_zero_header_size() {
        let mut data = b"mapsforge binary OSM".to_vec();
        data.extend_from_slice(&[0, 0, 0, 0]); // header_size = 0 (invalid)
        data.extend_from_slice(&[0, 0, 0, 3]); // version

        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        // Should fail validation
        assert!(result.is_err());
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_real_map_file_exists() {
        // First check if test file exists
        let file_exists = std::path::Path::new(TEST_FILE_PATH).exists();
        if !file_exists {
            println!("Warning: Test map file not found at {}", TEST_FILE_PATH);
            println!("Skipping real file test");
            return;
        }

        let file = File::open(TEST_FILE_PATH).expect("Failed to open test file");
        let mut reader = BufReader::new(file);
        let header = MapHeader::read_from_file(&mut reader).expect("Failed to parse header");

        // Basic assertions
        assert_eq!(header.magic, "mapsforge binary OSM");
        assert!(
            header.file_version >= 3,
            "File version should be at least 3"
        );
        assert!(header.tile_size > 0, "Tile size should be positive");
        assert!(
            header.num_zoom_intervals > 0,
            "Should have at least one zoom interval"
        );
    }

    #[test]
    fn test_real_map_file_bounding_box() {
        let file_exists = std::path::Path::new(TEST_FILE_PATH).exists();
        if !file_exists {
            return;
        }

        let file = File::open(TEST_FILE_PATH).unwrap();
        let mut reader = BufReader::new(file);
        let header = MapHeader::read_from_file(&mut reader).unwrap();

        // Validate bounding box
        let bbox = &header.bounding_box;
        assert!(bbox.min_lat >= -90.0 && bbox.min_lat <= 90.0);
        assert!(bbox.max_lat >= -90.0 && bbox.max_lat <= 90.0);
        assert!(bbox.min_lon >= -180.0 && bbox.min_lon <= 180.0);
        assert!(bbox.max_lon >= -180.0 && bbox.max_lon <= 180.0);
        assert!(bbox.min_lat <= bbox.max_lat);
        assert!(bbox.min_lon <= bbox.max_lon);
    }

    #[test]
    fn test_real_map_file_zoom_intervals() {
        let file_exists = std::path::Path::new(TEST_FILE_PATH).exists();
        if !file_exists {
            return;
        }

        let file = File::open(TEST_FILE_PATH).unwrap();
        let mut reader = BufReader::new(file);
        let header = MapHeader::read_from_file(&mut reader).unwrap();

        // Validate zoom intervals
        assert!(!header.zoom_interval_configuration.is_empty());

        for (i, interval) in header.zoom_interval_configuration.iter().enumerate() {
            assert!(
                interval.min_zoom_level <= interval.base_zoom_level,
                "Interval {}: min_zoom ({}) should be <= base_zoom ({})",
                i,
                interval.min_zoom_level,
                interval.base_zoom_level
            );
            assert!(
                interval.base_zoom_level <= interval.max_zoom_level,
                "Interval {}: base_zoom ({}) should be <= max_zoom ({})",
                i,
                interval.base_zoom_level,
                interval.max_zoom_level
            );
            assert!(
                interval.sub_file_size > 0,
                "Interval {}: sub_file_size should be positive",
                i
            );
            assert!(
                interval.sub_file_start > 0,
                "Interval {}: sub_file_start should be positive",
                i
            );
        }
    }

    #[test]
    fn test_real_map_file_tags() {
        let file_exists = std::path::Path::new(TEST_FILE_PATH).exists();
        if !file_exists {
            return;
        }

        let file = File::open(TEST_FILE_PATH).unwrap();
        let mut reader = BufReader::new(file);
        let header = MapHeader::read_from_file(&mut reader).unwrap();

        // Most map files should have tags
        println!("POI tags count: {}", header.poi_tags.len());
        println!("Way tags count: {}", header.way_tags.len());

        // Validate tag strings are not empty
        for tag in &header.poi_tags {
            assert!(!tag.is_empty(), "POI tag should not be empty");
        }

        for tag in &header.way_tags {
            assert!(!tag.is_empty(), "Way tag should not be empty");
        }
    }

    #[test]
    fn test_real_map_file_optional_fields() {
        let file_exists = std::path::Path::new(TEST_FILE_PATH).exists();
        if !file_exists {
            return;
        }

        let file = File::open(TEST_FILE_PATH).unwrap();
        let mut reader = BufReader::new(file);
        let header = MapHeader::read_from_file(&mut reader).unwrap();

        // Check optional fields based on flags
        if header.flags & 0x40 != 0 {
            assert!(
                header.map_start_position.is_some(),
                "Map start position should exist"
            );
            if let Some((lat, lon)) = header.map_start_position {
                assert!(lat >= -90.0 && lat <= 90.0);
                assert!(lon >= -180.0 && lon <= 180.0);
            }
        }

        if header.flags & 0x20 != 0 {
            assert!(
                header.start_zoom_level.is_some(),
                "Start zoom level should exist"
            );
        }

        if header.flags & 0x10 != 0 {
            assert!(
                header.language_preference.is_some(),
                "Language preference should exist"
            );
        }

        if header.flags & 0x08 != 0 {
            assert!(header.comment.is_some(), "Comment should exist");
        }

        if header.flags & 0x04 != 0 {
            assert!(header.created_by.is_some(), "Created by should exist");
        }
    }

    // ==================== Edge Case Tests ====================

    #[test]
    fn test_empty_file() {
        let data = vec![];
        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_too_short() {
        let data = vec![0u8; 10]; // Only 10 bytes, not enough for magic
        let mut reader = BufReader::new(Cursor::new(data));
        let result = MapHeader::read_from_file(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_vbe_encoding_edge_cases() -> Result<()> {
        // Test boundary between 1-byte and 2-byte encoding
        // Value 127 (max single byte)
        let data_127 = vec![0x7F];
        let mut reader = BufReader::new(Cursor::new(data_127));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 127);

        // Value 128 (requires 2 bytes)
        let data_128 = vec![0x80, 0x01];
        let mut reader = BufReader::new(Cursor::new(data_128));
        assert_eq!(MapHeader::read_vbe_u_int(&mut reader)?, 128);

        Ok(())
    }

    #[test]
    fn test_coordinate_precision() -> Result<()> {
        // Test that microdegrees conversion maintains precision
        // 50.123456 degrees = 50_123_456 microdegrees
        let microdegrees: i32 = 50_123_456;
        let degrees = microdegrees as f64 / 1_000_000.0;

        assert!((degrees - 50.123456).abs() < 0.000001);
        Ok(())
    }
}
