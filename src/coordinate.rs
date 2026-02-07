// coordinate.rs
use crate::types::BoundingBox;
use std::f64::consts::PI;

pub struct CoordinateConverter {
    pub zoom: u8,
    pub screen_width: u16,
    pub screen_height: u16,
    pub center_lat: f64,
    pub center_lon: f64,
    
    // Pre-calculated values for performance
    pixels_per_tile: f64,
    center_pixel_x: f64,
    center_pixel_y: f64,
}

impl CoordinateConverter {
    pub fn new(
        zoom: u8,
        screen_width: u16,
        screen_height: u16,
        center_lat: f64,
        center_lon: f64,
    ) -> Self {
        let pixels_per_tile = 256.0 * 2_f64.powi(zoom as i32);
        
        // Convert center lat/lon to pixel coordinates in world space
        let center_pixel_x = Self::lon_to_pixel_x(center_lon, zoom);
        let center_pixel_y = Self::lat_to_pixel_y(center_lat, zoom);
        
        Self {
            zoom,
            screen_width,
            screen_height,
            center_lat,
            center_lon,
            pixels_per_tile,
            center_pixel_x,
            center_pixel_y,
        }
    }
    
    /// Convert longitude to world pixel X coordinate (Web Mercator)
    fn lon_to_pixel_x(lon: f64, zoom: u8) -> f64 {
        let n = 2_f64.powi(zoom as i32);
        ((lon + 180.0) / 360.0) * n * 256.0
    }
    
    /// Convert latitude to world pixel Y coordinate (Web Mercator)
    fn lat_to_pixel_y(lat: f64, zoom: u8) -> f64 {
        let n = 2_f64.powi(zoom as i32);
        let lat_rad = lat.to_radians();
        let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0;
        y * n * 256.0
    }
    
    /// Convert a single lat/lon point to screen coordinates
    pub fn latlon_to_screen(&self, lat: f64, lon: f64) -> Option<(i16, i16)> {
        let world_x = Self::lon_to_pixel_x(lon, self.zoom);
        let world_y = Self::lat_to_pixel_y(lat, self.zoom);
        
        // Translate to screen coordinates (centered on viewport)
        let screen_x = (world_x - self.center_pixel_x) + (self.screen_width as f64 / 2.0);
        let screen_y = (world_y - self.center_pixel_y) + (self.screen_height as f64 / 2.0);
        
        // Check if point is visible on screen
        if screen_x < -1000.0 || screen_x > (self.screen_width as f64 + 1000.0) ||
           screen_y < -1000.0 || screen_y > (self.screen_height as f64 + 1000.0) {
            return None; // Way off screen, skip
        }
        
        Some((screen_x as i16, screen_y as i16))
    }
    
    /// Convert Way coordinates (microdegree offsets from tile origin) to screen pixels
    pub fn way_coords_to_screen(
        &self,
        tile_origin: (f64, f64), // (tile_lat, tile_lon)
        coords: &[(i32, i32)],    // (lat_offset_microdegrees, lon_offset_microdegrees)
    ) -> Vec<(i16, i16)> {
        coords
            .iter()
            .filter_map(|(lat_offset, lon_offset)| {
                // Convert microdegrees to degrees
                let lat = tile_origin.0 + (*lat_offset as f64 / 1_000_000.0);
                let lon = tile_origin.1 + (*lon_offset as f64 / 1_000_000.0);
                
                self.latlon_to_screen(lat, lon)
            })
            .collect()
    }
    
    /// Calculate the bounding box visible in current viewport
    pub fn get_viewport_bbox(&self) -> BoundingBox {
        // Calculate world pixel coordinates of viewport corners
        let top_left_world_x = self.center_pixel_x - (self.screen_width as f64 / 2.0);
        let top_left_world_y = self.center_pixel_y - (self.screen_height as f64 / 2.0);
        let bottom_right_world_x = self.center_pixel_x + (self.screen_width as f64 / 2.0);
        let bottom_right_world_y = self.center_pixel_y + (self.screen_height as f64 / 2.0);
        
        // Convert back to lat/lon
        let min_lon = Self::pixel_x_to_lon(top_left_world_x, self.zoom);
        let max_lat = Self::pixel_y_to_lat(top_left_world_y, self.zoom);
        let max_lon = Self::pixel_x_to_lon(bottom_right_world_x, self.zoom);
        let min_lat = Self::pixel_y_to_lat(bottom_right_world_y, self.zoom);
        
        BoundingBox {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        }
    }
    
    /// Convert world pixel X back to longitude
    fn pixel_x_to_lon(pixel_x: f64, zoom: u8) -> f64 {
        let n = 2_f64.powi(zoom as i32);
        (pixel_x / (n * 256.0)) * 360.0 - 180.0
    }
    
    /// Convert world pixel Y back to latitude
    fn pixel_y_to_lat(pixel_y: f64, zoom: u8) -> f64 {
        let n = 2_f64.powi(zoom as i32);
        let y = pixel_y / (n * 256.0);
        let lat_rad = (PI * (1.0 - 2.0 * y)).sinh().atan();
        lat_rad.to_degrees()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_coordinate_conversion() {
        // Test with San Francisco coordinates
        let converter = CoordinateConverter::new(
            13,     // zoom
            320,    // screen width
            240,    // screen height
            37.7749, // San Francisco latitude
            -122.4194, // San Francisco longitude
        );
        
        // Center point should map to center of screen
        let (x, y) = converter.latlon_to_screen(37.7749, -122.4194).unwrap();
        assert!((x - 160).abs() < 2); // Should be near center X (320/2)
        assert!((y - 120).abs() < 2); // Should be near center Y (240/2)
    }
}