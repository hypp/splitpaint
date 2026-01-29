use crate::canvas::Canvas;
use crate::types::*;
use eframe::egui;
use egui::{Color32, Stroke};

pub struct PixelArtApp {
    pub canvas: Canvas,
    pub zoom: f32,
    pub active_layer: Layer,
    pub active_tool: Tool,
    
    // Pixel layer state
    drawing: bool,
    draw_value: bool,
    draw_prev_pos: Option<(usize, usize)>,
    line_start: Option<(usize, usize)>,
    
    // Raster split layer state
    dragging_split: Option<u64>,
    editing_split: Option<(i32, usize)>,
    edit_color: [u8; 3],
    hovered_split: Option<(i32, usize)>,
    
    // Add split dialog
    show_add_dialog: bool,
    dialog_scanline: i32,
    dialog_x: i32,
    dialog_channel: ColorChannel,
    dialog_color: [u8; 3],
    
    // Cursor colors
    cursor_color0: Color,
    cursor_color1: Color,
    
    // UI state
    status_message: String,
}

impl Default for PixelArtApp {
    fn default() -> Self {
        Self {
            canvas: Canvas::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            zoom: 2.0,
            active_layer: Layer::Pixels,
            active_tool: Tool::Pencil,
            drawing: false,
            draw_value: true,
            line_start: None,
            draw_prev_pos: None,
            dragging_split: None,
            editing_split: None,
            edit_color: [0xFF, 0xFF, 0xFF],
            hovered_split: None,
            show_add_dialog: false,
            dialog_scanline: 0,
            dialog_x: 0,
            dialog_channel: ColorChannel::Color1,
            dialog_color: [0xFF, 0x00, 0x00],
            cursor_color0: Color::new(0, 0, 0),
            cursor_color1: Color::new(255, 255, 255),
            status_message: String::new(),
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
                        ui.close_menu();
                    }
                    
                    if ui.add_enabled(self.canvas.can_redo(), egui::Button::new("Redo"))
                        .on_hover_text("Ctrl+Y or Ctrl+Shift+Z")
                        .clicked() 
                    {
                        self.canvas.redo();
                        ui.close_menu();
                    }
                    
                    ui.separator();
                    
                    if ui.button("Clear Canvas").clicked() {
                        self.canvas.pixels = vec![false; self.canvas.width * self.canvas.height];
                        self.canvas.push_undo_state();  // Spara EFTER clear
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("View", |ui| {
                    ui.label("Zoom controls coming soon...");
                });
            });
        });
        
        // Keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && !i.modifiers.shift) {
            self.canvas.undo();
        }
        if ctx.input(|i| (i.key_pressed(egui::Key::Y) && i.modifiers.ctrl) || 
                         (i.key_pressed(egui::Key::Z) && i.modifiers.ctrl && i.modifiers.shift)) {
            self.canvas.redo();
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
                
                ui.separator();
                ui.label("⌨️ Shortcuts: P=Pencil | E=Eraser | L=Line | 1/2=Layers | Ctrl+Z=Undo | Ctrl+Y=Redo");
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
            
            if !self.status_message.is_empty() {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, &self.status_message);
            }
        });
        
        // Add split dialog
        if self.show_add_dialog {
            egui::Window::new("Add Raster Split")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("Scanline: {}, X: {}", self.dialog_scanline, self.dialog_x));
                    
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.dialog_channel, ColorChannel::Color0, "Color 0");
                        ui.selectable_value(&mut self.dialog_channel, ColorChannel::Color1, "Color 1");
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_add_dialog = false;
                        }
                        if ui.button("Add Split").clicked() {
                            let color = Color::new(self.dialog_color[0], self.dialog_color[1], self.dialog_color[2]);
                            match self.canvas.add_raster_split(self.dialog_scanline, self.dialog_x, self.dialog_channel, color) {
                                Ok(_) => {
                                    self.canvas.push_undo_state();  // Spara efter att split lagts till
                                    self.status_message = "Split added!".to_string();
                                },
                                Err(e) => self.status_message = e,
                            }
                            self.show_add_dialog = false;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Color:");
                        if ui.color_edit_button_srgb(&mut self.dialog_color).changed() {
                            let snapped = Color::snap_to_amiga(
                                self.dialog_color[0],
                                self.dialog_color[1],
                                self.dialog_color[2]
                            );
                            self.dialog_color = [snapped.r, snapped.g, snapped.b];
                        }
                    });
                });
        }
        
        // Edit split dialog
        if let Some((scanline, index)) = self.editing_split {
            egui::Window::new("Edit Split")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if let Some(splits) = self.canvas.raster_splits.get(&scanline) {
                        if let Some(split) = splits.get(index) {
                            ui.label(format!("Line: {}, Copper: {}", scanline, split.copper_x));
                            
                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                if ui.color_edit_button_srgb(&mut self.edit_color).changed() {
                                    let snapped = Color::snap_to_amiga(
                                        self.edit_color[0],
                                        self.edit_color[1],
                                        self.edit_color[2]
                                    );
                                    self.edit_color = [snapped.r, snapped.g, snapped.b];
                                }
                            });
                            
                            ui.horizontal(|ui| {
                                if ui.button("Cancel").clicked() {
                                    self.editing_split = None;
                                }
                                if ui.button("Apply").clicked() {
                                    let color = Color::new(self.edit_color[0], self.edit_color[1], self.edit_color[2]);
                                    self.canvas.update_split_color(scanline, index, color);
                                    self.canvas.push_undo_state();  // Spara efter färgändring
                                    self.editing_split = None;
                                    self.status_message = "Split updated".to_string();
                                }
                            });
                        }
                    }
                });
        }
        
        // Canvas
        self.render_canvas(ctx);
    }
}

impl PixelArtApp {
    fn render_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(
                egui::vec2(
                    (self.canvas.width as i32 + BORDER_SIZE * 2) as f32 * self.zoom,
                    (self.canvas.height as i32 + BORDER_SIZE * 2) as f32 * self.zoom
                ),
                egui::Sense::click_and_drag(),
            );

            let canvas_rect = response.rect;
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
            
            // Handle interactions
            self.handle_interactions(&response, mouse_pos, mouse_x, mouse_y);
            
            // Draw border
            self.draw_border(&painter, &canvas_rect);
            
            // Draw pixel layer
            if self.canvas.show_pixel_layer {
                self.draw_pixels(&painter, &canvas_rect, mouse_x, mouse_y);
            }
            
            // Draw raster split layer
            if self.canvas.show_raster_layer {
                self.draw_splits(&painter, &canvas_rect);
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
                                self.draw_prev_pos = Some((mouse_x as usize, mouse_y as usize))
                            }
                        }
                        if response.dragged() {
                            if mouse_pos.is_some() {
                                if let Some((x0, y0)) = self.draw_prev_pos {
                                    self.canvas.draw_line(x0, y0, mouse_x as usize, mouse_y as usize, self.draw_value);
                                    self.draw_prev_pos = Some((mouse_x as usize, mouse_y as usize))
                                }
                            }
                        }
                        if response.drag_released() {
                            self.drawing = false;
                            // Spara det modifierade tillståndet EFTER vi slutat rita
                            self.canvas.push_undo_state();
                        }
                    }
                    Tool::Line => {
                        if response.drag_started() {
                            self.line_start = Some((mouse_x as usize, mouse_y as usize));
                            self.draw_value = self.active_tool == Tool::Pencil;
                        }
                        
                        if response.drag_released() {
                            if let Some((x0, y0)) = self.line_start {
                                self.canvas.draw_line(x0, y0, mouse_x as usize, mouse_y as usize, self.draw_value);
                                self.canvas.push_undo_state();  // Spara EFTER
                                self.line_start = None;
                            }
                        }
                    }
                }
            }
            
            Layer::RasterSplits => {
                if response.secondary_clicked() {
                    if let Some((scanline, index)) = self.canvas.find_split_at(mouse_x, mouse_y, 9) {
                        self.canvas.remove_raster_split(scanline, index);
                        self.canvas.push_undo_state();  // Spara efter borttagning
                        self.status_message = "Split deleted".to_string();
                    }
                }

                if mouse_pos.is_some() && self.dragging_split.is_none() {
                    self.hovered_split = self.canvas.find_split_at(mouse_x, mouse_y, 9);
                } else if self.dragging_split.is_none() {
                    self.hovered_split = None;
                }

                if response.drag_started() {
                    if let Some((scanline, index)) = self.hovered_split {
                        // Hämta split-ID
                        if let Some(splits) = self.canvas.raster_splits.get(&scanline) {
                            if let Some(split) = splits.get(index) {
                                self.dragging_split = Some(split.id);
                            }
                        }
                    }
                }                

                if let Some(split_id) = self.dragging_split {
                    if response.dragged() {
                        // Hitta splitsen med detta ID
                        let mut found: Option<(i32, usize)> = None;
                        for (scanline, splits) in &self.canvas.raster_splits {
                            for (i, split) in splits.iter().enumerate() {
                                if split.id == split_id {
                                    found = Some((*scanline, i));
                                    break;
                                }
                            }
                            if found.is_some() {
                                break;
                            }
                        }
                        
                        if let Some((current_scanline, current_index)) = found {
                            let desired_copper = Canvas::pixel_to_copper(mouse_x);
                            
                            let (valid_scanline, valid_copper) = self.canvas.find_nearest_valid_position(
                                mouse_y, 
                                desired_copper, 
                                current_scanline, 
                                current_index
                            );
                            
                            let valid_pixel = Canvas::copper_to_pixel(valid_copper);
                            
                            let _ = self.canvas.move_raster_split(current_scanline, current_index, valid_scanline, valid_pixel);
                        }
                    }
                    
                    if response.drag_released() {
                        self.dragging_split = None;
                        self.canvas.push_undo_state();  // Spara EFTER drag
                    }
                }

                if response.clicked() && self.dragging_split.is_none() {
                    if let Some((scanline, index)) = self.hovered_split {
                        if let Some(splits) = self.canvas.raster_splits.get(&scanline) {
                            if let Some(split) = splits.get(index) {
                                self.editing_split = Some((scanline, index));
                                self.edit_color = [split.color.r, split.color.g, split.color.b];
                            }
                        }
                    } else {
                        self.show_add_dialog = true;
                        self.dialog_scanline = mouse_y;
                        self.dialog_x = mouse_x;
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
    
    fn draw_pixels(&self, painter: &egui::Painter, canvas_rect: &egui::Rect, mouse_x: i32, mouse_y: i32) {
        for y in 0..self.canvas.height {
            for x in 0..self.canvas.width {
                let color = self.canvas.get_color_at(x, y);
                let rect = egui::Rect::from_min_size(
                    canvas_rect.min + egui::vec2((x as i32 + BORDER_SIZE) as f32 * self.zoom, (y as i32 + BORDER_SIZE) as f32 * self.zoom),
                    egui::vec2(self.zoom, self.zoom),
                );
                painter.rect_filled(rect, 0.0, color.to_egui_color32());
            }
        }

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
    
    fn draw_splits(&self, painter: &egui::Painter, canvas_rect: &egui::Rect) {
        for (scanline, splits) in &self.canvas.raster_splits {
            if *scanline < -BORDER_SIZE || *scanline >= (self.canvas.height as i32 + BORDER_SIZE) {
                continue;
            }
            
            for (i, split) in splits.iter().enumerate() {
                let is_hovered = self.hovered_split == Some((*scanline, i));
                let is_dragging = self.dragging_split == Some(split.id);
                
                let draw_pixel_x = Canvas::copper_to_pixel(split.copper_x);
                let draw_y = *scanline;

                let x_screen = canvas_rect.left() + (draw_pixel_x + BORDER_SIZE) as f32 * self.zoom;
                let y_top = canvas_rect.top() + (draw_y + BORDER_SIZE) as f32 * self.zoom;
                let y_bottom = y_top + self.zoom;
                
                let color = if is_hovered || is_dragging {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::from_rgb(255, 128, 0)
                };
                
                let stroke = egui::Stroke::new(if is_hovered { 3.0 } else { 2.0 }, color);
                
                painter.line_segment(
                    [egui::pos2(x_screen, y_top), egui::pos2(x_screen, y_bottom)],
                    stroke,
                );
                
                painter.circle_filled(
                    egui::pos2(x_screen, y_top + self.zoom / 2.0),
                    if is_hovered { 4.0 } else { 3.0 },
                    color,
                );
            }
        }
    }
}