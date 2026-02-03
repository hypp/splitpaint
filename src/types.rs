use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::atomic::{AtomicU64, Ordering}};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub const DEFAULT_WIDTH: usize = 320;
pub const DEFAULT_HEIGHT: usize = 256;
pub const COPPER_WAIT_DISTANCE: usize = 8;
#[allow(dead_code)]
pub const COPPER_WAIT_POS: usize = 4;
pub const BORDER_SIZE: i32 = 66;

#[derive(Clone, Copy, PartialEq)]
pub enum Layer {
    Pixels,
    RasterSplits,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Pencil,
    Eraser,
    Line,
    Eyedropper,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    
    pub fn snap_to_amiga(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: ((r >> 4) << 4) | (r >> 4),
            g: ((g >> 4) << 4) | (g >> 4),
            b: ((b >> 4) << 4) | (b >> 4),
        }
    }
    
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
    
    pub fn to_egui_color32(&self) -> eframe::egui::Color32 {
        eframe::egui::Color32::from_rgb(self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize,Debug)]
pub enum ColorChannel {
    Color0,
    Color1,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RasterSplit {
    #[serde(skip, default = "RasterSplit::next_id")]
    #[allow(dead_code)]
    pub id: u64,
    #[serde(skip)]
    pub scanline: i32,
    pub copper_x: u8,
    pub channel: ColorChannel,
    pub color: Color,
}

impl RasterSplit {
    pub fn new(scanline: i32, copper_x: u8, channel: ColorChannel, color: Color) -> Self {
        Self { id: Self::next_id(), scanline, copper_x, channel, color }
    }

    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ProjectData {
    pub width: usize,
    pub height: usize,
    pub raster_splits: std::collections::BTreeMap<i32, Vec<RasterSplit>>,
}

pub struct CanvasState {
    pub pixels: Vec<bool>,
    pub raster_splits: BTreeMap<i32, Vec<RasterSplit>>,
}

pub struct UndoHistory {
    states: Vec<CanvasState>,
    current_index: usize,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self { states: Vec::new(), current_index: 0 }
    }

    pub fn push(&mut self, state: CanvasState) {
        // If we've undone and then do something new, remove future states
        if !self.states.is_empty() {
            self.states.truncate(self.current_index + 1);
        }
        
        self.states.push(state);
        self.current_index = self.states.len() - 1;
        
        // Limit history (e.g. 50 levels)
        if self.states.len() > 50 {
            self.states.remove(0);
            self.current_index = self.current_index.saturating_sub(1);
        }
    }
    
    pub fn undo(&mut self) -> Option<&CanvasState> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(&self.states[self.current_index])
        } else {
            None
        }
    }
    
    pub fn redo(&mut self) -> Option<&CanvasState> {
        if !self.states.is_empty() && self.current_index < self.states.len() - 1 {
            self.current_index += 1;
            Some(&self.states[self.current_index])
        } else {
            None
        }
    }
    
    pub fn can_undo(&self) -> bool {
        self.current_index > 0 && !self.states.is_empty()
    }
    
    pub fn can_redo(&self) -> bool {
        !self.states.is_empty() && self.current_index < self.states.len() - 1
    }
    
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}