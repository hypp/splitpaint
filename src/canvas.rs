use crate::types::*;
use std::collections::BTreeMap;
use std::fs;

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<bool>,
    pub raster_splits: BTreeMap<i32, Vec<RasterSplit>>,
    pub show_pixel_layer: bool,
    pub show_raster_layer: bool,
    occupied_positions: std::collections::HashSet<(i32, u8)>,
    history: UndoHistory,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        let mut canvas = Self {
            width,
            height,
            pixels: vec![false; width * height],
            raster_splits: BTreeMap::new(),
            show_pixel_layer: true,
            show_raster_layer: true,
            occupied_positions: std::collections::HashSet::new(),
            history: UndoHistory::new(),
        };
        
        // Spara initial empty state
        canvas.push_undo_state();
        
        canvas
    }
    
    pub fn pixel_to_copper(pixel_x: i32) -> u8 {
        let adjusted = pixel_x + BORDER_SIZE;
        ((adjusted / 4) * 2 + 1).clamp(1, 225) as u8
    }
    
    pub fn copper_to_pixel(copper_x: u8) -> i32 {
        (((copper_x as i32 - 1) / 2) * 4) - BORDER_SIZE
    }
    
    pub fn get_pixel(&self, x: usize, y: usize) -> bool {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            false
        }
    }
    
    pub fn set_pixel(&mut self, x: usize, y: usize, value: bool) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = value;
        }
    }
    
    pub fn get_last_color(&self, channel: ColorChannel, default: Color) -> Color {
        for (_scanline, splits) in self.raster_splits.iter().rev() {
            for split in splits.iter().rev() {
                if split.channel == channel {
                    return split.color;
                }
            }
        }
        default
    }

    pub fn get_active_colors(&self, scanline: i32, pixel_x: i32) -> (Color, Color) {
        let mut current_color0 = self.get_last_color(ColorChannel::Color0, Color::new(0, 0, 0));
        let mut current_color1 = self.get_last_color(ColorChannel::Color1, Color::new(255, 255, 255));
        
        for (y, splits) in &self.raster_splits {
            if y > &scanline {
                break;
            } 
            
            for split in splits {
                let split_pixel_x = Self::copper_to_pixel(split.copper_x);
                
                if y < &scanline || (y == &scanline && pixel_x >= split_pixel_x) {
                    match split.channel {
                        ColorChannel::Color0 => current_color0 = split.color,
                        ColorChannel::Color1 => current_color1 = split.color,
                    }
                }
            }
        }
        
        (current_color0, current_color1)
    }

    pub fn get_color_at(&self, x: usize, y: usize) -> Color {
        let (color0, color1) = self.get_active_colors(y as i32, x as i32);
        if self.get_pixel(x, y) {
            color1
        } else {
            color0
        }
    }

    fn mark_occupied(&mut self, scanline: i32, copper_x: u8) {
        let min_gap = (COPPER_WAIT_DISTANCE / 4) as i32 * 2;
        for offset in (-min_gap)..=min_gap {
            let blocked = (copper_x as i32 + offset).clamp(1, 225) as u8;
            self.occupied_positions.insert((scanline, blocked));
        }
    }
    
    fn unmark_occupied(&mut self, scanline: i32, copper_x: u8) {
        let min_gap = (COPPER_WAIT_DISTANCE / 4) as i32 * 2;
        for offset in (-min_gap)..=min_gap {
            let blocked = (copper_x as i32 + offset).clamp(1, 225) as u8;
            self.occupied_positions.remove(&(scanline, blocked));
        }
    }    


    pub fn can_place_split(&self, scanline: i32, copper_x: u8, exclude_index: Option<usize>) -> bool {
        if let Some(splits) = self.raster_splits.get(&scanline) {
            for (i, split) in splits.iter().enumerate() {
                if Some(i) == exclude_index {
                    continue;
                }
                
                if split.copper_x == copper_x {
                    return false;
                }
                
                let pixel_distance = (Self::copper_to_pixel(copper_x) - Self::copper_to_pixel(split.copper_x)).abs();
                if pixel_distance < COPPER_WAIT_DISTANCE as i32 {
                    return false;
                }
            }
        }
        true
    }
    


    pub fn set_raster_split(&mut self, scanline: i32, pixel_x: i32, channel: ColorChannel, color: Color) -> Result<(), String> {
        let copper_x = Self::pixel_to_copper(pixel_x);
        
        // Remove all splits within 4 pixels on same scanline (any channel)
        if let Some(splits) = self.raster_splits.get_mut(&scanline) {
            let mut indices_to_remove = Vec::new();
            
            for (i, split) in splits.iter().enumerate() {
                let pixel_distance = (Self::copper_to_pixel(copper_x) - Self::copper_to_pixel(split.copper_x)).abs();
                if pixel_distance <= 4 {
                    indices_to_remove.push((i, split.copper_x));
                }
            }
            
            // Remove in reverse order to preserve indices
            for &(index, _) in indices_to_remove.iter().rev() {
                splits.remove(index);
            }
            
            if splits.is_empty() {
                self.raster_splits.remove(&scanline);
            }

            for &(_, removed_copper) in indices_to_remove.iter().rev() {
                self.unmark_occupied(scanline, removed_copper);
            }

        }
        
        // Now add the new split
        self.mark_occupied(scanline, copper_x);
        
        self.raster_splits
            .entry(scanline)
            .or_insert_with(Vec::new)
            .push(RasterSplit::new(scanline, copper_x, channel, color));
        
        if let Some(splits) = self.raster_splits.get_mut(&scanline) {
            splits.sort_by_key(|s| s.copper_x);
        }
        
        Ok(())
    }
    
    pub fn clear_raster_split(&mut self, scanline: i32, pixel_x: i32, channel: ColorChannel) -> bool {
        // Konvertera till copper position
        let copper_x = Self::pixel_to_copper(pixel_x);
        
        // Find and remove split at this position and channel
        let (removed_copper, is_empty) = if let Some(splits) = self.raster_splits.get_mut(&scanline) {
            if let Some(index) = splits.iter().position(|s| s.copper_x == copper_x && s.channel == channel) {
                let copper = splits[index].copper_x;
                splits.remove(index);
                (Some(copper), splits.is_empty())
            } else {
                (None, false)
            }
        } else {
            (None, false)
        };
        
        // Nu kan vi mutera self utan konflikt
        if let Some(copper) = removed_copper {
            self.unmark_occupied(scanline, copper);
            
            if is_empty {
                self.raster_splits.remove(&scanline);
            }
            
            true
        } else {
            false
        }
    }
    


    pub fn draw_line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, value: bool) {
        let dx = (x1 as i32 - x0 as i32).abs();
        let dy = (y1 as i32 - y0 as i32).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        
        let mut x = x0 as i32;
        let mut y = y0 as i32;
        
        loop {
            self.set_pixel(x as usize, y as usize, value);
            
            if x == x1 as i32 && y == y1 as i32 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn draw_raster_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, channel: ColorChannel, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            let _ = self.set_raster_split(y, x, channel, color);
            
            if x == x1 && y == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }
    
    pub fn erase_raster_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, channel: ColorChannel) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx - dy;
        
        let mut x = x0;
        let mut y = y0;
        
        loop {
            self.clear_raster_split(y, x, channel);
            
            if x == x1 && y == y1 {
                break;
            }
            
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn push_undo_state(&mut self) {
        let state = CanvasState { pixels: self.pixels.clone(), raster_splits: self.raster_splits.clone()};
        self.history.push(state);
    }
    
    pub fn undo(&mut self) {
        if let Some(state) = self.history.undo() {
            self.pixels = state.pixels.clone();
            self.raster_splits = state.raster_splits.clone();
            self.rebuild_occupied_positions();
        }
    }
    
    pub fn redo(&mut self) {
        if let Some(state) = self.history.redo() {
            self.pixels = state.pixels.clone();
            self.raster_splits = state.raster_splits.clone();
            self.rebuild_occupied_positions();
        }
    }
    
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    
    fn rebuild_occupied_positions(&mut self) {
        self.occupied_positions.clear();
        
        // Collect all positions first, then mark them
        let positions: Vec<(i32, u8)> = self.raster_splits
            .iter()
            .flat_map(|(scanline, splits)| {
                splits.iter().map(move |split| (*scanline, split.copper_x))
            })
            .collect();
        
        for (scanline, copper_x) in positions {
            self.mark_occupied(scanline, copper_x);
        }
    }
    
    pub fn export_bitplane_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        for y in 0..self.height {
            for x in (0..self.width).step_by(8) {
                let mut byte = 0u8;
                for bit in 0..8 {
                    if x + bit < self.width && self.get_pixel(x + bit, y) {
                        byte |= 1 << (7 - bit);
                    }
                }
                data.push(byte);
            }
        }
        
        data
    }
    
    pub fn import_bitplane_data(&mut self, data: &[u8]) -> Result<(), String> {
        let expected_size = (self.width * self.height) / 8;
        if data.len() != expected_size {
            return Err(format!("Expected {} bytes, got {}", expected_size, data.len()));
        }
        
        let mut idx = 0;
        for y in 0..self.height {
            for x in (0..self.width).step_by(8) {
                let byte = data[idx];
                idx += 1;
                
                for bit in 0..8 {
                    if x + bit < self.width {
                        let pixel_set = (byte & (1 << (7 - bit))) != 0;
                        self.set_pixel(x + bit, y, pixel_set);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    pub fn import_png(&mut self, path: &str) -> Result<(), String> {
        let img = image::open(path)
            .map_err(|e| format!("Failed to open PNG: {}", e))?;
        
        let img = img.to_rgb8();
        
        // Clear current pixels
        self.pixels = vec![false; self.width * self.height];
        
        // Import pixels - treat any non-black pixel as "on"
        for y in 0..self.height.min(img.height() as usize) {
            for x in 0..self.width.min(img.width() as usize) {
                let pixel = img.get_pixel(x as u32, y as u32);
                // If pixel is not black, set it
                let is_set = pixel[0] > 128 || pixel[1] > 128 || pixel[2] > 128;
                self.set_pixel(x, y, is_set);
            }
        }
        
        Ok(())
    }
    
    pub fn save_project(&self, json_path: &str, bin_path: &str) -> Result<(), String> {
        let project = ProjectData {
            width: self.width,
            height: self.height,
            raster_splits: self.raster_splits.clone(),
        };
        
        let json = serde_json::to_string_pretty(&project)
            .map_err(|e| format!("JSON error: {}", e))?;
        
        fs::write(json_path, json)
            .map_err(|e| format!("Write error: {}", e))?;
        
        let bitplane_data = self.export_bitplane_data();
        fs::write(bin_path, bitplane_data)
            .map_err(|e| format!("Write error: {}", e))?;
        
        Ok(())
    }
    
    pub fn load_project(&mut self, json_path: Option<&str>, bin_path: Option<&str>) -> Result<(), String> {
        if let Some(path) = json_path {
            let json = fs::read_to_string(path)
                .map_err(|e| format!("Read error: {}", e))?;
            
            let mut project: ProjectData = serde_json::from_str(&json)
                .map_err(|e| format!("JSON error: {}", e))?;

            // Fix scanline field for loaded splits
            for (scanline, splits) in &mut project.raster_splits {
                for split in splits {
                    split.scanline = *scanline;
                }
            }

            self.width = project.width;
            self.height = project.height;
            self.raster_splits = project.raster_splits;
            self.pixels = vec![false; self.width * self.height];
        }
        
        if let Some(path) = bin_path {
            let data = fs::read(path)
                .map_err(|e| format!("Read error: {}", e))?;
            
            self.import_bitplane_data(&data)?;
        }
        
        // Clear undo history and save loaded project as first state
        self.history = UndoHistory::new();
        self.push_undo_state();
        
        Ok(())
    }
    
    pub fn export_png(&self, filename: &str) -> Result<(), String> {
        let mut img = image::RgbImage::new(self.width as u32, self.height as u32);
        
        for y in 0..self.height {
            for x in 0..self.width {
                let color = self.get_color_at(x, y);
                img.put_pixel(x as u32, y as u32, image::Rgb([color.r, color.g, color.b]));
            }
        }
        
        img.save(filename).map_err(|e| format!("PNG error: {}", e))
    }


}