use crate::canvas::Canvas;
use crate::types::*;
use eframe::egui;
use egui::{Color32, Stroke};

pub struct PixelArtApp {
    pub canvas: Canvas,
    pub zoom: f32,
    pub pan_offset: (f32, f32), // (x, y) offset for panning
    pub active_layer: Layer,
    pub active_tool: Tool,
    
    // Pixel layer state
    drawing: bool,
    draw_value: bool,
    draw_prev_pos: Option<(usize, usize)>,
    line_start: Option<(usize, usize)>,
    
    // Paint split state
    paint_split_color: [u8; 3],
    paint_split_channel: ColorChannel,
    painting_splits: bool,
    paint_prev_pos: Option<(i32, i32)>,
    
    // Cursor colors
    cursor_color0: Color,
    cursor_color1: Color,
    
    // UI state
    status_message: String,
    
    // View settings
    show_grid: bool,
    grid_size: usize,
    
    // Cached rendering
    cached_image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
    dirty_rect: Option<(usize, usize, usize, usize)>,
    pending_dirty_pixels: Vec<(usize, usize)>,
    last_render_time: std::time::Instant,
}

impl Default for PixelArtApp {
    fn default() -> Self {
        Self {
            canvas: Canvas::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            zoom: 2.0,
            pan_offset: (0.0, 0.0),
            active_layer: Layer::Pixels,
            active_tool: Tool::Pencil,
            drawing: false,
            draw_value: true,
            line_start: None,
            draw_prev_pos: None,
            paint_split_color: [0xFF, 0x00, 0x00],
            paint_split_channel: ColorChannel::Color1,
            painting_splits: false,
            paint_prev_pos: None,
            cursor_color0: Color::new(0, 0, 0),
            cursor_color1: Color::new(255, 255, 255),
            status_message: String::new(),
            show_grid: false,
            grid_size: 8,
            cached_image: None,
            texture: None,
            dirty_rect: Some((0, 0, DEFAULT_WIDTH, DEFAULT_HEIGHT)),
            pending_dirty_pixels: Vec::new(),
            last_render_time: std::time::Instant::now(),
        }
    }
}

impl eframe::App for PixelArtApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save Project...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_file_name("project.json")
                            .save_file()
                        {
                            let json_path = path.to_string_lossy().to_string();
                            let bin_path = json_path.replace(".json", ".bin");
                            
                            match self.canvas.save_project(&json_path, &bin_path) {
                                Ok(_) => self.status_message = "Saved!".to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                    
                    if ui.button("Load Project...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            let json_path = path.to_string_lossy().to_string();
                            let bin_path = json_path.replace(".json", ".bin");
                            
                            match self.canvas.load_project(Some(&json_path), Some(&bin_path)) {
                                Ok(_) => self.status_message = "Loaded!".to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button("Import PNG...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .pick_file()
                        {
                            match self.canvas.import_png(&path.to_string_lossy()) {
                                Ok(_) => {
                                    self.canvas.push_undo_state();  // Spara efter import
                                    self.status_message = "PNG imported!".to_string();
                                },
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                    
                    if ui.button("Export PNG...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .set_file_name("output.png")
                            .save_file()
                        {
                            match self.canvas.export_png(&path.to_string_lossy()) {
                                Ok(_) => self.status_message = "PNG exported!".to_string(),
                                Err(e) => self.status_message = e,
                            }
                        }
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("Edit", |ui| {
                    if ui.add_enabled(self.canvas.can_undo(), egui::Button::new("Undo"))
                        .on_hover_text("Ctrl+Z")
                        .clicked() 
                    {
                        self.canvas.undo();
                        self.mark_all_dirty();
                        ui.close_menu();
                    }
                    
                    if ui.add_enabled(self.canvas.can_redo(), egui::Button::new("Redo"))
                        .on_hover_text("Ctrl+Y or Ctrl+Shift+Z")
                        .clicked() 
                    {
                        self.canvas.redo();
                        self.mark_all_dirty();
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button("Clear Canvas").clicked() {
                        self.canvas.pixels = vec![false; self.canvas.width * self.canvas.height];
                        self.canvas.push_undo_state();
                        self.mark_all_dirty();
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Zoom:");
                        if ui.button("-").clicked() {
                            self.zoom = (self.zoom - 1.0).max(1.0);
                        }
                        ui.label(format!("{}x", self.zoom as i32));
                        if ui.button("+").clicked() {
                            self.zoom = (self.zoom + 1.0).min(16.0);
                        }
                    });
                    
                    ui.separator();
                    
                    ui.checkbox(&mut self.show_grid, "Show Grid");
                    if self.show_grid {
                        ui.horizontal(|ui| {
                            ui.label("Grid size:");
                            ui.radio_value(&mut self.grid_size, 4, "4x4");
                            ui.radio_value(&mut self.grid_size, 8, "8x8");
                        });
                    }
                    
                    ui.separator();
                    
                    ui.checkbox(&mut self.canvas.show_pixel_layer, "Show Pixels");
                    ui.checkbox(&mut self.canvas.show_raster_layer, "Show Raster Splits");
                });
            });
        });
        
        // Keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && !i.modifiers.shift) {
            self.canvas.undo();
            self.mark_all_dirty();
        }
        if ctx.input(|i| (i.key_pressed(egui::Key::Y) && i.modifiers.ctrl) || 
                         (i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && i.modifiers.shift)) {
            self.canvas.redo();
            self.mark_all_dirty();
        }
        
        // Tool shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.active_tool = Tool::Pencil;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::E)) {
            self.active_tool = Tool::Eraser;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L)) {
            self.active_tool = Tool::Line;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::I)) {
            self.active_tool = Tool::Eyedropper;
        }
        
        // Layer shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
            self.active_layer = Layer::Pixels;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
            self.active_layer = Layer::RasterSplits;
        }
        
        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.active_tool == Tool::Pencil, "✏️ Pencil").clicked() {
                    self.active_tool = Tool::Pencil;
                }
                if ui.selectable_label(self.active_tool == Tool::Eraser, "🧹 Eraser").clicked() {
                    self.active_tool = Tool::Eraser;
                }
                if ui.selectable_label(self.active_tool == Tool::Line, "📏 Line").clicked() {
                    self.active_tool = Tool::Line;
                }
                if ui.selectable_label(self.active_tool == Tool::Eyedropper, "💧 Eyedropper").clicked() {
                    self.active_tool = Tool::Eyedropper;
                }
                
                ui.separator();
                ui.label("⌨️ Shortcuts: P=Pencil | E=Eraser | L=Line | I=Eyedropper | 1/2=Layers | Ctrl+Z=Undo | Ctrl+Y=Redo");
            });
        });
        
        // Sidebar
        egui::SidePanel::left("controls").min_width(260.0).show(ctx, |ui| {
            ui.separator();
            ui.label("Layers");
            
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_layer, Layer::Pixels, "Pixels");
                ui.selectable_value(&mut self.active_layer, Layer::RasterSplits, "Raster");
            });
            
            ui.checkbox(&mut self.canvas.show_pixel_layer, "Show Pixel Layer");
            ui.checkbox(&mut self.canvas.show_raster_layer, "Show Raster Layer");
            
            ui.separator();
            ui.label("Colors at Cursor");
            
            ui.horizontal(|ui| {
                ui.label("Color 0:");
                let _color_widget = egui::widgets::color_picker::show_color(
                    ui, 
                    self.cursor_color0.to_egui_color32(), 
                    egui::Vec2::new(30.0, 20.0)
                );
                ui.label(self.cursor_color0.to_hex());
            });
            
            ui.horizontal(|ui| {
                ui.label("Color 1:");
                let _color_widget = egui::widgets::color_picker::show_color(
                    ui, 
                    self.cursor_color1.to_egui_color32(), 
                    egui::Vec2::new(30.0, 20.0)
                );
                ui.label(self.cursor_color1.to_hex());
            });
            
            // Raster Split controls
            if self.active_layer == Layer::RasterSplits {
                ui.separator();
                ui.label("Raster Split Paint");
                
                ui.horizontal(|ui| {
                    ui.label("Channel:");
                    ui.selectable_value(&mut self.paint_split_channel, ColorChannel::Color0, "Color 0");
                    ui.selectable_value(&mut self.paint_split_channel, ColorChannel::Color1, "Color 1");
                });
                
                ui.horizontal(|ui| {
                    ui.label("Color:");
                    if ui.color_edit_button_srgb(&mut self.paint_split_color).changed() {
                        let snapped = Color::snap_to_amiga(
                            self.paint_split_color[0],
                            self.paint_split_color[1],
                            self.paint_split_color[2]
                        );
                        self.paint_split_color = [snapped.r, snapped.g, snapped.b];
                    }
                });
                
                ui.label("Pencil: Paint 8px color strips");
                ui.label("Eraser: Remove splits");
                ui.label("Line: Draw straight splits");
            }
            
            if !self.status_message.is_empty() {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, &self.status_message);
            }
        });
        
        // Canvas
        self.render_canvas(ctx);
    }
}

impl PixelArtApp {
    fn mark_dirty_pixel(&mut self, x: usize, y: usize) {
        // Add to pending buffer instead of updating dirty_rect immediately
        self.pending_dirty_pixels.push((x, y));
    }
    
    fn flush_pending_dirty_pixels(&mut self) {
        // Flytta alla pending pixlar till dirty_rect
        for (x, y) in self.pending_dirty_pixels.drain(..) {
            if let Some((min_x, min_y, max_x, max_y)) = self.dirty_rect {
                self.dirty_rect = Some((
                    min_x.min(x),
                    min_y.min(y),
                    max_x.max(x + 1),
                    max_y.max(y + 1),
                ));
            } else {
                self.dirty_rect = Some((x, y, x + 1, y + 1));
            }
        }
    }
    
    fn mark_dirty_scanline(&mut self, _scanline: i32) {
        // Mark entire screen as dirty when a split changes
        // (simpler and correct, rendering happens async so it matters less)
        self.dirty_rect = Some((0, 0, self.canvas.width, self.canvas.height));
    }
    
    fn mark_all_dirty(&mut self) {
        self.dirty_rect = Some((0, 0, self.canvas.width, self.canvas.height));
    }
    
    fn render_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Handle zoom FIRST, before allocating painter
            let zoom_in = ui.input(|i| {
                i.events.iter().any(|e| {
                    if let egui::Event::Text(text) = e {
                        text == "+" || text == "="
                    } else {
                        false
                    }
                })
            });
            
            let zoom_out = ui.input(|i| {
                i.events.iter().any(|e| {
                    if let egui::Event::Text(text) = e {
                        text == "-"
                    } else {
                        false
                    }
                })
            });
            
            if zoom_in || zoom_out {
                // Get mouse position BEFORE zoom
                if let Some(mouse_screen) = ui.input(|i| i.pointer.hover_pos()) {
                    // We need to calculate the old canvas_rect to get canvas coordinates
                    let old_rect_min = egui::pos2(
                        ui.available_rect_before_wrap().left() + self.pan_offset.0,
                        ui.available_rect_before_wrap().top() + self.pan_offset.1
                    );
                    
                    // Convert screen pos to canvas coordinates with current zoom
                    let canvas_x_before = (mouse_screen.x - old_rect_min.x) / self.zoom;
                    let canvas_y_before = (mouse_screen.y - old_rect_min.y) / self.zoom;
                    
                    if zoom_in {
                        self.zoom = (self.zoom + 1.0).min(16.0);
                    } else {
                        self.zoom = (self.zoom - 1.0).max(1.0);
                    }
                    
                    // Calculate where the same canvas point would be with new zoom
                    let canvas_x_after_screen = canvas_x_before * self.zoom;
                    let canvas_y_after_screen = canvas_y_before * self.zoom;
                    
                    // Adjust pan so mouse stays on same canvas point
                    let mouse_x_relative = mouse_screen.x - ui.available_rect_before_wrap().left();
                    let mouse_y_relative = mouse_screen.y - ui.available_rect_before_wrap().top();
                    
                    self.pan_offset.0 = mouse_x_relative - canvas_x_after_screen;
                    self.pan_offset.1 = mouse_y_relative - canvas_y_after_screen;
                }
            }
            
            let (response, painter) = ui.allocate_painter(
                ui.available_size(),
                egui::Sense::click_and_drag(),
            );

            // Calculate canvas size with current zoom
            let canvas_pixel_size = egui::vec2(
                (self.canvas.width as i32 + BORDER_SIZE * 2) as f32 * self.zoom,
                (self.canvas.height as i32 + BORDER_SIZE * 2) as f32 * self.zoom
            );
            
            // Center canvas if it's smaller than viewport
            let mut offset = egui::vec2(self.pan_offset.0, self.pan_offset.1);
            if canvas_pixel_size.x < response.rect.width() {
                offset.x = (response.rect.width() - canvas_pixel_size.x) / 2.0;
            }
            if canvas_pixel_size.y < response.rect.height() {
                offset.y = (response.rect.height() - canvas_pixel_size.y) / 2.0;
            }
            
            let canvas_rect = egui::Rect::from_min_size(
                response.rect.min + offset,
                canvas_pixel_size,
            );
            
            // Handle panning with middle mouse button or space+drag
            let is_panning = ui.input(|i| {
                i.pointer.middle_down() || (i.key_down(egui::Key::Space) && i.pointer.primary_down())
            });
            
            if is_panning {
                if let Some(delta) = ui.input(|i| {
                    if i.pointer.is_decidedly_dragging() {
                        Some(i.pointer.delta())
                    } else {
                        None
                    }
                }) {
                    self.pan_offset.0 += delta.x;
                    self.pan_offset.1 += delta.y;
                }
            }
            let mouse_pos = response.hover_pos();
            
            let (mouse_x, mouse_y) = if let Some(pos) = mouse_pos {
                let x = ((pos.x - canvas_rect.left()) / self.zoom) as i32 - BORDER_SIZE;
                let y = ((pos.y - canvas_rect.top()) / self.zoom) as i32 - BORDER_SIZE;
                
                if self.active_layer == Layer::RasterSplits {
                    (x.max(-BORDER_SIZE).min(self.canvas.width as i32 + BORDER_SIZE - 1),
                     y.max(-BORDER_SIZE).min(self.canvas.height as i32 + BORDER_SIZE - 1))
                } else {
                    (x.max(0).min(self.canvas.width as i32 - 1), 
                     y.max(0).min(self.canvas.height as i32 - 1))
                }
            } else {
                (0, 0)
            };
            
            // Update cursor colors
            if mouse_pos.is_some() {
                let (c0, c1) = self.canvas.get_active_colors(mouse_y, mouse_x);
                self.cursor_color0 = c0;
                self.cursor_color1 = c1;
            }
            
            // Handle interactions (skip if panning)
            if !is_panning {
                self.handle_interactions(&response, mouse_pos, mouse_x, mouse_y);
            }
            
            // Draw border
            self.draw_border(&painter, &canvas_rect);
            
            // Draw pixel layer
            if self.canvas.show_pixel_layer {
                self.draw_pixels(&painter, &canvas_rect, mouse_x, mouse_y, ctx);
            }
            
            // Draw grid
            if self.show_grid {
                self.draw_grid(&painter, &canvas_rect);
            }
            
            // Draw raster split layer
            if self.canvas.show_raster_layer {
                self.draw_splits(&painter, &canvas_rect);
            }
            
            // Draw raster split preview
            if self.active_layer == Layer::RasterSplits && mouse_pos.is_some() {
                match self.active_tool {
                    Tool::Pencil | Tool::Eraser => {
                        self.draw_paint_split_preview(&painter, &canvas_rect, mouse_x, mouse_y);
                    }
                    Tool::Line => {
                        // Show line preview while dragging
                        if let Some((x0, y0)) = self.line_start {
                            self.draw_split_line_preview(&painter, &canvas_rect, x0 as i32, y0 as i32, mouse_x, mouse_y);
                        } else {
                            self.draw_paint_split_preview(&painter, &canvas_rect, mouse_x, mouse_y);
                        }
                    }
                    Tool::Eyedropper => {
                        // Show cursor crosshair for eyedropper
                        let pixel_rect = egui::Rect::from_min_size(
                            canvas_rect.min + egui::vec2((mouse_x + BORDER_SIZE) as f32 * self.zoom, (mouse_y + BORDER_SIZE) as f32 * self.zoom),
                            egui::vec2(self.zoom, self.zoom)
                        );
                        painter.rect_stroke(pixel_rect, 0.0, Stroke::new(2.0, Color32::WHITE));
                    }
                }
            }
        });
    }
    
    fn handle_interactions(&mut self, response: &egui::Response, mouse_pos: Option<egui::Pos2>, mouse_x: i32, mouse_y: i32) {
        match self.active_layer {
            Layer::Pixels => {
                match self.active_tool {
                    Tool::Pencil | Tool::Eraser => {
                        if response.drag_started() || response.clicked() {
                            if mouse_pos.is_some() {
                                if !self.drawing {
                                    self.draw_value = self.active_tool == Tool::Pencil;
                                    self.drawing = true;
                                }
                                self.canvas.set_pixel(mouse_x as usize, mouse_y as usize, self.draw_value);
                                self.mark_dirty_pixel(mouse_x as usize, mouse_y as usize);
                                self.draw_prev_pos = Some((mouse_x as usize, mouse_y as usize))
                            }
                        }
                        if response.dragged() {
                            if mouse_pos.is_some() {
                                if let Some((x0, y0)) = self.draw_prev_pos {
                                    let drawn = self.canvas.draw_line(x0, y0, mouse_x as usize, mouse_y as usize, self.draw_value);
                                    for (x, y) in drawn {
                                        self.mark_dirty_pixel(x, y);
                                    }
                                    self.draw_prev_pos = Some((mouse_x as usize, mouse_y as usize))
                                }
                            }
                        }
                        if response.drag_released() {
                            self.drawing = false;
                            self.flush_pending_dirty_pixels(); // Rendera alla pending omedelbart
                            self.canvas.push_undo_state();
                        }
                    }
                    Tool::Line => {
                        if response.drag_started() {
                            self.line_start = Some((mouse_x as usize, mouse_y as usize));
                        }
                        
                        if response.drag_released() {
                            if let Some((x0, y0)) = self.line_start {
                                let drawn = self.canvas.draw_line(x0, y0, mouse_x as usize, mouse_y as usize, true);
                                for (x, y) in drawn {
                                    self.mark_dirty_pixel(x, y);
                                }
                                self.canvas.push_undo_state();
                                self.line_start = None;
                            }
                        }
                    }
                    Tool::Eyedropper => {
                        if response.clicked() && mouse_pos.is_some() {
                            // Sample color at cursor - nothing to do for pixels layer
                            // (color is already shown in cursor_color0/cursor_color1)
                        }
                    }
                }
            }
            
            Layer::RasterSplits => {
                match self.active_tool {
                    Tool::Pencil => {
                        if response.drag_started() || response.clicked() {
                            if mouse_pos.is_some() {
                                if !self.painting_splits {
                                    self.painting_splits = true;
                                }
                                let color = Color::new(
                                    self.paint_split_color[0],
                                    self.paint_split_color[1],
                                    self.paint_split_color[2]
                                );
                                let _ = self.canvas.set_raster_split(mouse_y, mouse_x, self.paint_split_channel, color);
                                self.mark_dirty_scanline(mouse_y);
                                self.paint_prev_pos = Some((mouse_x, mouse_y));
                            }
                        } 
                        if response.dragged() {
                            if mouse_pos.is_some() {
                                if let Some((prev_x, prev_y)) = self.paint_prev_pos {
                                    let color = Color::new(
                                        self.paint_split_color[0],
                                        self.paint_split_color[1],
                                        self.paint_split_color[2]
                                    );
                                    
                                    // Draw line from prev to current
                                    let dx = (mouse_x - prev_x).abs();
                                    let dy = (mouse_y - prev_y).abs();
                                    let sx = if prev_x < mouse_x { 1 } else { -1 };
                                    let sy = if prev_y < mouse_y { 1 } else { -1 };
                                    let mut err = dx - dy;
                                    
                                    let mut x = prev_x;
                                    let mut y = prev_y;
                                    let mut last_scanline = prev_y;
                                    
                                    loop {
                                        let copper_x = Canvas::pixel_to_copper(x);
                                        
                                        // If we're on the same scanline as previous point, add directly
                                        // Otherwise use set_raster_split to clear nearby splits first
                                        if y == last_scanline {
                                            self.canvas.add_raster_split_direct(y, copper_x, self.paint_split_channel, color);
                                        } else {
                                            let _ = self.canvas.set_raster_split(y, x, self.paint_split_channel, color);
                                            last_scanline = y;
                                        }
                                        
                                        if x == mouse_x && y == mouse_y {
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
                                    
                                    // Mark all scanlines between prev_y and mouse_y as dirty
                                    let min_y = prev_y.min(mouse_y);
                                    let max_y = prev_y.max(mouse_y);
                                    for y in min_y..=max_y {
                                        self.mark_dirty_scanline(y);
                                    }
                                    self.paint_prev_pos = Some((mouse_x, mouse_y));
                                }
                            }
                        }
                        if response.drag_released() {
                            self.painting_splits = false;
                            self.paint_prev_pos = None;
                            self.flush_pending_dirty_pixels();
                            self.canvas.push_undo_state();
                        }
                    }
                    Tool::Eraser => {
                        if response.drag_started() || response.clicked() {
                            if mouse_pos.is_some() {
                                if !self.painting_splits {
                                    self.painting_splits = true;
                                }
                                // Erase first point
                                self.canvas.clear_raster_split(mouse_y, mouse_x);
                                self.mark_dirty_scanline(mouse_y);
                                self.paint_prev_pos = Some((mouse_x, mouse_y));
                            }
                        }
                        
                        // Erase even if mouse has moved (without explicit drag event)
                        if self.painting_splits && mouse_pos.is_some() {
                            if let Some((prev_x, prev_y)) = self.paint_prev_pos {
                                // Only erase if mouse actually moved
                                if prev_x != mouse_x || prev_y != mouse_y {
                                    self.canvas.erase_raster_line(prev_x, prev_y, mouse_x, mouse_y);
                                    let min_y = prev_y.min(mouse_y);
                                    let max_y = prev_y.max(mouse_y);
                                    for y in min_y..=max_y {
                                        self.mark_dirty_scanline(y);
                                    }
                                    self.paint_prev_pos = Some((mouse_x, mouse_y));
                                }
                            }
                        }
                        
                        if response.drag_released() {
                            self.painting_splits = false;
                            self.paint_prev_pos = None;
                            self.flush_pending_dirty_pixels();
                            self.canvas.push_undo_state();
                        }
                    }
                    Tool::Line => {
                        if response.drag_started() {
                            self.line_start = Some((mouse_x as usize, mouse_y as usize));
                        }
                        
                        if response.drag_released() {
                            if let Some((x0, y0)) = self.line_start {
                                let color = Color::new(
                                    self.paint_split_color[0],
                                    self.paint_split_color[1],
                                    self.paint_split_color[2]
                                );
                                self.canvas.draw_raster_line(x0 as i32, y0 as i32, mouse_x, mouse_y, self.paint_split_channel, color);
                                
                                // Mark all affected scanlines as dirty
                                let min_y = (y0 as i32).min(mouse_y);
                                let max_y = (y0 as i32).max(mouse_y);
                                for y in min_y..=max_y {
                                    self.mark_dirty_scanline(y);
                                }
                                
                                self.canvas.push_undo_state();
                                self.line_start = None;
                            }
                        }
                    }
                    Tool::Eyedropper => {
                        if response.clicked() && mouse_pos.is_some() {
                            // Sample color at cursor
                            let sampled_color = match self.paint_split_channel {
                                ColorChannel::Color0 => self.cursor_color0,
                                ColorChannel::Color1 => self.cursor_color1,
                            };
                            self.paint_split_color = [sampled_color.r, sampled_color.g, sampled_color.b];
                            self.status_message = format!("Sampled color: {}", sampled_color.to_hex());
                        }
                    }
                }
            }
        }
    }
    
    fn draw_border(&self, painter: &egui::Painter, canvas_rect: &egui::Rect) {
        for y in -BORDER_SIZE..(self.canvas.height as i32 + BORDER_SIZE) {
            for x in -BORDER_SIZE..(self.canvas.width as i32 + BORDER_SIZE) {
                if x >= 0 && x < self.canvas.width as i32 && y >= 0 && y < self.canvas.height as i32 {
                    continue;
                }
                
                let (color0, _) = self.canvas.get_active_colors(y, x);
                let x_screen = canvas_rect.left() + (x + BORDER_SIZE) as f32 * self.zoom;
                let y_screen = canvas_rect.top() + (y + BORDER_SIZE) as f32 * self.zoom;
                
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x_screen, y_screen),
                    egui::vec2(self.zoom, self.zoom)
                );
                painter.rect_filled(rect, 0.0, color0.to_egui_color32());
            }
        }
    }
    
    fn draw_grid(&self, painter: &egui::Painter, canvas_rect: &egui::Rect) {
        let grid_color = Color32::from_rgba_premultiplied(255, 255, 255, 40);
        
        // Vertical lines
        for x in (0..self.canvas.width).step_by(self.grid_size) {
            let x_screen = canvas_rect.left() + (x as i32 + BORDER_SIZE) as f32 * self.zoom;
            let y_start = canvas_rect.top() + BORDER_SIZE as f32 * self.zoom;
            let y_end = y_start + self.canvas.height as f32 * self.zoom;
            painter.line_segment(
                [egui::pos2(x_screen, y_start), egui::pos2(x_screen, y_end)],
                Stroke::new(1.0, grid_color),
            );
        }
        
        // Horizontal lines
        for y in (0..self.canvas.height).step_by(self.grid_size) {
            let y_screen = canvas_rect.top() + (y as i32 + BORDER_SIZE) as f32 * self.zoom;
            let x_start = canvas_rect.left() + BORDER_SIZE as f32 * self.zoom;
            let x_end = x_start + self.canvas.width as f32 * self.zoom;
            painter.line_segment(
                [egui::pos2(x_start, y_screen), egui::pos2(x_end, y_screen)],
                Stroke::new(1.0, grid_color),
            );
        }
    }
    
    fn draw_pixels(&mut self, painter: &egui::Painter, canvas_rect: &egui::Rect, mouse_x: i32, mouse_y: i32, ctx: &egui::Context) {
        // Initialize cache if needed
        if self.cached_image.is_none() {
            self.cached_image = Some(egui::ColorImage::new(
                [self.canvas.width, self.canvas.height],
                egui::Color32::BLACK,
            ));
        }
        
        // Flush pending dirty pixels to dirty_rect
        if !self.pending_dirty_pixels.is_empty() {
            self.flush_pending_dirty_pixels();
        }
        
        // Check if we should render (throttle to ~30fps during drawing, instant otherwise)
        let now = std::time::Instant::now();
        let time_since_last = now.duration_since(self.last_render_time).as_millis();
        let is_drawing = self.drawing || self.painting_splits;
        let should_render = self.dirty_rect.is_some() && 
                           (!is_drawing || time_since_last >= 33); // 33ms = ~30fps
        
        // Render dirty region if needed
        if should_render {
            if let Some((min_x, min_y, max_x, max_y)) = self.dirty_rect.take() {
                if let Some(image) = &mut self.cached_image {
                    for y in min_y..max_y.min(self.canvas.height) {
                        // Get active colors at start of this scanline
                        let (mut current_color0, mut current_color1) = self.canvas.get_active_colors(y as i32, 0);
                        
                        // Get splits on this scanline
                        let mut split_changes = Vec::new();
                        if let Some(splits) = self.canvas.raster_splits.get(&(y as i32)) {
                            for split in splits {
                                let split_pixel_x = Canvas::copper_to_pixel(split.copper_x);
                                split_changes.push((split_pixel_x, split.channel, split.color));
                            }
                        }
                        
                        // Render this scanline
                        let mut change_idx = 0;
                        for x in min_x..max_x.min(self.canvas.width) {
                            // Apply any color changes at this x position
                            while change_idx < split_changes.len() && split_changes[change_idx].0 <= x as i32 {
                                match split_changes[change_idx].1 {
                                    ColorChannel::Color0 => current_color0 = split_changes[change_idx].2,
                                    ColorChannel::Color1 => current_color1 = split_changes[change_idx].2,
                                }
                                change_idx += 1;
                            }
                            
                            let color = if self.canvas.get_pixel(x, y) {
                                current_color1.to_egui_color32()
                            } else {
                                current_color0.to_egui_color32()
                            };
                            
                            image.pixels[y * self.canvas.width + x] = color;
                        }
                    }
                    
                    // Update texture
                    self.texture = Some(ctx.load_texture(
                        "canvas",
                        image.clone(),
                        egui::TextureOptions::NEAREST,
                    ));
                    
                    self.last_render_time = now;
                }
            }
        }
        
        // Request repaint if we still have dirty data
        if self.dirty_rect.is_some() || !self.pending_dirty_pixels.is_empty() {
            ctx.request_repaint();
        }
        
        // Draw cached texture
        if let Some(texture) = &self.texture {
            let texture_rect = egui::Rect::from_min_size(
                canvas_rect.min + egui::vec2(BORDER_SIZE as f32 * self.zoom, BORDER_SIZE as f32 * self.zoom),
                egui::vec2(self.canvas.width as f32 * self.zoom, self.canvas.height as f32 * self.zoom),
            );
            
            painter.image(
                texture.id(),
                texture_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        
        // Show line preview only on Pixels layer
        if self.active_layer == Layer::Pixels {
            if let Some((x0, y0)) = self.line_start {
                let start = egui::pos2(
                    canvas_rect.left() + (x0 as i32 + BORDER_SIZE) as f32 * self.zoom,
                    canvas_rect.top() + (y0 as i32 + BORDER_SIZE) as f32 * self.zoom
                );
                let end = egui::pos2(
                    canvas_rect.left() + (mouse_x + BORDER_SIZE) as f32 * self.zoom,
                    canvas_rect.top() + (mouse_y + BORDER_SIZE) as f32 * self.zoom
                );
                painter.line_segment([start, end], Stroke::new(2.0, Color32::WHITE));
            }
        }
    }
    
    fn draw_splits(&self, painter: &egui::Painter, canvas_rect: &egui::Rect) {
        for (scanline, splits) in &self.canvas.raster_splits {
            if *scanline < -BORDER_SIZE || *scanline >= (self.canvas.height as i32 + BORDER_SIZE) {
                continue;
            }
            
            for split in splits.iter() {
                let draw_pixel_x = Canvas::copper_to_pixel(split.copper_x);
                let draw_y = *scanline;

                let x_screen = canvas_rect.left() + (draw_pixel_x + BORDER_SIZE) as f32 * self.zoom;
                let y_screen = canvas_rect.top() + (draw_y + BORDER_SIZE) as f32 * self.zoom;
                
                // Draw 8-pixel wide colored bar
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x_screen, y_screen),
                    egui::vec2(8.0 * self.zoom, self.zoom)
                );
                
                // Draw the color from split
                painter.rect_filled(rect, 0.0, split.color.to_egui_color32());
            }
        }
    }
    
    fn draw_paint_split_preview(&self, painter: &egui::Painter, canvas_rect: &egui::Rect, mouse_x: i32, mouse_y: i32) {
        // Konvertera till copper position
        let copper_x = Canvas::pixel_to_copper(mouse_x);
        let pixel_x = Canvas::copper_to_pixel(copper_x);
        
        // Draw a 8-pixel wide preview bar
        let x_screen = canvas_rect.left() + (pixel_x + BORDER_SIZE) as f32 * self.zoom;
        let y_screen = canvas_rect.top() + (mouse_y + BORDER_SIZE) as f32 * self.zoom;
        
        let preview_color = Color::new(
            self.paint_split_color[0],
            self.paint_split_color[1],
            self.paint_split_color[2]
        );
        
        // Rita 8 pixlar bred vertikal bar
        for i in 0..8 {
            let x = x_screen + i as f32 * self.zoom;
            painter.line_segment(
                [egui::pos2(x, y_screen), egui::pos2(x, y_screen + self.zoom)],
                egui::Stroke::new(self.zoom, preview_color.to_egui_color32().gamma_multiply(0.7)),
            );
        }
        
        // Draw copper position indicator at left edge
        painter.circle_stroke(
            egui::pos2(x_screen, y_screen + self.zoom / 2.0),
            6.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );
    }
    
    fn draw_split_line_preview(&self, painter: &egui::Painter, canvas_rect: &egui::Rect, x0: i32, y0: i32, x1: i32, y1: i32) {
        // Draw a preview of the line that will be created
        let start_copper = Canvas::pixel_to_copper(x0);
        let end_copper = Canvas::pixel_to_copper(x1);
        let start_pixel = Canvas::copper_to_pixel(start_copper);
        let end_pixel = Canvas::copper_to_pixel(end_copper);
        
        let start_screen = egui::pos2(
            canvas_rect.left() + (start_pixel + BORDER_SIZE) as f32 * self.zoom + 4.0 * self.zoom,
            canvas_rect.top() + (y0 + BORDER_SIZE) as f32 * self.zoom + self.zoom / 2.0
        );
        let end_screen = egui::pos2(
            canvas_rect.left() + (end_pixel + BORDER_SIZE) as f32 * self.zoom + 4.0 * self.zoom,
            canvas_rect.top() + (y1 + BORDER_SIZE) as f32 * self.zoom + self.zoom / 2.0
        );
        
        let preview_color = Color::new(
            self.paint_split_color[0],
            self.paint_split_color[1],
            self.paint_split_color[2]
        );
        
        painter.line_segment(
            [start_screen, end_screen],
            egui::Stroke::new(3.0, preview_color.to_egui_color32()),
        );
    }
}