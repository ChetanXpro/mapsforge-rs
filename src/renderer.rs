// renderer.rs
use std::path::Path;

pub struct MapRenderer {
    framebuffer: Vec<u16>, // RGB565 format
    width: u16,
    height: u16,
}

// RGB565 color helpers
pub mod colors {
    pub const WHITE: u16 = 0xFFFF;
    pub const BLACK: u16 = 0x0000;
    pub const RED: u16 = 0xF800;
    pub const GREEN: u16 = 0x07E0;
    pub const BLUE: u16 = 0x001F;
    pub const YELLOW: u16 = 0xFFE0;
    pub const CYAN: u16 = 0x07FF;
    pub const MAGENTA: u16 = 0xF81F;
    
    // Map-specific colors
    pub const WATER: u16 = 0x3D7F;      // Light blue
    pub const LAND: u16 = 0xEF5D;       // Beige/tan
    pub const ROAD_MAJOR: u16 = 0xFE60; // Orange
    pub const ROAD_MINOR: u16 = 0xFFFF; // White
    pub const BUILDING: u16 = 0xC618;   // Gray
    pub const PARK: u16 = 0x5D63;       // Green
    
    /// Convert RGB888 to RGB565
    pub fn rgb(r: u8, g: u8, b: u8) -> u16 {
        let r5 = (r as u16 >> 3) & 0x1F;
        let g6 = (g as u16 >> 2) & 0x3F;
        let b5 = (b as u16 >> 3) & 0x1F;
        (r5 << 11) | (g6 << 5) | b5
    }
    
    /// Convert RGB565 to RGB888 for PNG export
    pub fn rgb565_to_rgb888(color: u16) -> (u8, u8, u8) {
        let r = ((color >> 11) & 0x1F) as u8;
        let g = ((color >> 5) & 0x3F) as u8;
        let b = (color & 0x1F) as u8;
        
        // Scale to 8-bit
        (
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
        )
    }
}

impl MapRenderer {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            framebuffer: vec![colors::WHITE; (width as usize) * (height as usize)],
            width,
            height,
        }
    }
    
    pub fn clear(&mut self, color: u16) {
        self.framebuffer.fill(color);
    }
    
    /// Set a single pixel (with bounds checking)
    fn set_pixel(&mut self, x: i16, y: i16, color: u16) {
        if x >= 0 && x < self.width as i16 && y >= 0 && y < self.height as i16 {
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            self.framebuffer[idx] = color;
        }
    }
    
    /// Draw a line using Bresenham's algorithm
    pub fn draw_line(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, color: u16) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            self.set_pixel(x, y, color);
            
            if x == x1 && y == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
    
    /// Draw a thick line (simple approach: draw multiple parallel lines)
    pub fn draw_thick_line(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, color: u16, thickness: u8) {
        if thickness == 1 {
            self.draw_line(x0, y0, x1, y1, color);
            return;
        }
        
        let half_thickness = (thickness / 2) as i16;
        
        // Draw parallel lines for thickness
        for offset in -half_thickness..=half_thickness {
            // Determine if line is more horizontal or vertical
            let dx = (x1 - x0).abs();
            let dy = (y1 - y0).abs();
            
            if dx > dy {
                // More horizontal, offset in Y
                self.draw_line(x0, y0 + offset, x1, y1 + offset, color);
            } else {
                // More vertical, offset in X
                self.draw_line(x0 + offset, y0, x1 + offset, y1, color);
            }
        }
    }
    
    /// Draw a polyline (connected line segments)
    pub fn draw_way(&mut self, screen_coords: &[(i16, i16)], color: u16, thickness: u8) {
        if screen_coords.len() < 2 {
            return;
        }
        
        for i in 0..screen_coords.len() - 1 {
            let (x0, y0) = screen_coords[i];
            let (x1, y1) = screen_coords[i + 1];
            self.draw_thick_line(x0, y0, x1, y1, color, thickness);
        }
    }
    
    /// Fill a polygon using scanline algorithm
    pub fn fill_polygon(&mut self, screen_coords: &[(i16, i16)], color: u16) {
        if screen_coords.len() < 3 {
            return; // Need at least 3 points for a polygon
        }
        
        // Find Y bounds
        let min_y = screen_coords.iter().map(|(_, y)| *y).min().unwrap().max(0);
        let max_y = screen_coords.iter().map(|(_, y)| *y).max().unwrap().min(self.height as i16 - 1);
        
        // Scanline fill
        for y in min_y..=max_y {
            let mut intersections = Vec::new();
            
            // Find intersections with polygon edges
            for i in 0..screen_coords.len() {
                let j = (i + 1) % screen_coords.len();
                let (x0, y0) = screen_coords[i];
                let (x1, y1) = screen_coords[j];
                
                // Check if edge crosses scanline
                if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                    // Calculate X intersection
                    let x = x0 + ((y - y0) * (x1 - x0)) / (y1 - y0);
                    intersections.push(x);
                }
            }
            
            // Sort intersections
            intersections.sort_unstable();
            
            // Fill between pairs of intersections
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    let x_start = pair[0].max(0);
                    let x_end = pair[1].min(self.width as i16 - 1);
                    
                    for x in x_start..=x_end {
                        self.set_pixel(x, y, color);
                    }
                }
            }
        }
    }
    
    /// Draw a circle (for GPS position indicator)
    pub fn draw_circle(&mut self, cx: i16, cy: i16, radius: u8, color: u16, filled: bool) {
        let r = radius as i16;
        
        if filled {
            for y in -r..=r {
                for x in -r..=r {
                    if x * x + y * y <= r * r {
                        self.set_pixel(cx + x, cy + y, color);
                    }
                }
            }
        } else {
            // Midpoint circle algorithm for outline
            let mut x = 0;
            let mut y = r;
            let mut d = 3 - 2 * r;
            
            while y >= x {
                self.set_pixel(cx + x, cy + y, color);
                self.set_pixel(cx - x, cy + y, color);
                self.set_pixel(cx + x, cy - y, color);
                self.set_pixel(cx - x, cy - y, color);
                self.set_pixel(cx + y, cy + x, color);
                self.set_pixel(cx - y, cy + x, color);
                self.set_pixel(cx + y, cy - x, color);
                self.set_pixel(cx - y, cy - x, color);
                
                x += 1;
                if d > 0 {
                    y -= 1;
                    d = d + 4 * (x - y) + 10;
                } else {
                    d = d + 4 * x + 6;
                }
            }
        }
    }
    
    /// Export framebuffer as PNG (requires `png` crate)
    pub fn save_as_png(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::BufWriter;
        
        let file = File::create(path)?;
        let w = BufWriter::new(file);
        
        let mut encoder = png::Encoder::new(w, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        
        let mut writer = encoder.write_header()?;
        
        // Convert RGB565 to RGB888
        let mut rgb_data = Vec::with_capacity((self.width as usize) * (self.height as usize) * 3);
        for &pixel in &self.framebuffer {
            let (r, g, b) = colors::rgb565_to_rgb888(pixel);
            rgb_data.push(r);
            rgb_data.push(g);
            rgb_data.push(b);
        }
        
        writer.write_image_data(&rgb_data)?;
        Ok(())
    }
}