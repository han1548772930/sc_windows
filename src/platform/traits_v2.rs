// Improved Platform Traits with Better Abstraction Levels
//
// Separates low-level primitive operations from high-level UI operations

use crate::error::{PlatformError, AppResult};
use std::any::Any;

/// Color definition
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Color = Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }
}

/// Point definition
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Rectangle definition
#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    
    pub fn from_bounds(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
    
    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.x && 
        point.x <= self.x + self.width &&
        point.y >= self.y &&
        point.y <= self.y + self.height
    }
}

/// Text style configuration
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font_size: f32,
    pub color: Color,
    pub font_family: String,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            color: Color::BLACK,
            font_family: "Segoe UI".to_string(),
            bold: false,
            italic: false,
        }
    }
}

/// Drawing style configuration
#[derive(Debug, Clone)]
pub struct DrawStyle {
    pub stroke_color: Option<Color>,
    pub fill_color: Option<Color>,
    pub stroke_width: f32,
    pub dash_pattern: Option<Vec<f32>>,
}

impl Default for DrawStyle {
    fn default() -> Self {
        Self {
            stroke_color: Some(Color::BLACK),
            fill_color: None,
            stroke_width: 1.0,
            dash_pattern: None,
        }
    }
}

/// Low-level primitive rendering operations
pub trait PrimitiveRenderer {
    /// Begin a new rendering frame
    fn begin_frame(&mut self) -> AppResult<()>;
    
    /// End the current rendering frame
    fn end_frame(&mut self) -> AppResult<()>;
    
    /// Clear the entire canvas with a color
    fn clear(&mut self, color: Color) -> AppResult<()>;
    
    /// Draw a line between two points
    fn draw_line(&mut self, start: Point, end: Point, style: &DrawStyle) -> AppResult<()>;
    
    /// Draw a rectangle
    fn draw_rectangle(&mut self, rect: Rectangle, style: &DrawStyle) -> AppResult<()>;
    
    /// Draw a circle
    fn draw_circle(&mut self, center: Point, radius: f32, style: &DrawStyle) -> AppResult<()>;
    
    /// Draw a polygon from a series of points
    fn draw_polygon(&mut self, points: &[Point], style: &DrawStyle) -> AppResult<()>;
    
    /// Draw text at a position
    fn draw_text(&mut self, text: &str, position: Point, style: &TextStyle) -> AppResult<()>;
    
    /// Measure text dimensions
    fn measure_text(&self, text: &str, style: &TextStyle) -> AppResult<(f32, f32)>;
    
    /// Set a clipping rectangle
    fn push_clip(&mut self, rect: Rectangle) -> AppResult<()>;
    
    /// Remove the current clipping rectangle
    fn pop_clip(&mut self) -> AppResult<()>;
}

/// High-level UI rendering operations
pub trait UIRenderer: PrimitiveRenderer {
    /// Draw a selection mask (darkens everything except the selection area)
    fn draw_selection_mask(
        &mut self,
        screen_rect: Rectangle,
        selection_rect: Rectangle,
        mask_opacity: f32,
    ) -> AppResult<()> {
        // Default implementation using primitives
        let mask_color = Color::new(0.0, 0.0, 0.0, mask_opacity);
        
        // Draw four rectangles around the selection
        // Top
        if selection_rect.y > screen_rect.y {
            self.draw_rectangle(
                Rectangle::new(
                    screen_rect.x,
                    screen_rect.y,
                    screen_rect.width,
                    selection_rect.y - screen_rect.y,
                ),
                &DrawStyle {
                    fill_color: Some(mask_color),
                    stroke_color: None,
                    ..Default::default()
                },
            )?;
        }
        
        // Bottom
        let selection_bottom = selection_rect.y + selection_rect.height;
        let screen_bottom = screen_rect.y + screen_rect.height;
        if selection_bottom < screen_bottom {
            self.draw_rectangle(
                Rectangle::new(
                    screen_rect.x,
                    selection_bottom,
                    screen_rect.width,
                    screen_bottom - selection_bottom,
                ),
                &DrawStyle {
                    fill_color: Some(mask_color),
                    stroke_color: None,
                    ..Default::default()
                },
            )?;
        }
        
        // Left
        if selection_rect.x > screen_rect.x {
            self.draw_rectangle(
                Rectangle::new(
                    screen_rect.x,
                    selection_rect.y,
                    selection_rect.x - screen_rect.x,
                    selection_rect.height,
                ),
                &DrawStyle {
                    fill_color: Some(mask_color),
                    stroke_color: None,
                    ..Default::default()
                },
            )?;
        }
        
        // Right
        let selection_right = selection_rect.x + selection_rect.width;
        let screen_right = screen_rect.x + screen_rect.width;
        if selection_right < screen_right {
            self.draw_rectangle(
                Rectangle::new(
                    selection_right,
                    selection_rect.y,
                    screen_right - selection_right,
                    selection_rect.height,
                ),
                &DrawStyle {
                    fill_color: Some(mask_color),
                    stroke_color: None,
                    ..Default::default()
                },
            )?;
        }
        
        Ok(())
    }
    
    /// Draw selection handles (resize grips)
    fn draw_selection_handles(
        &mut self,
        rect: Rectangle,
        handle_size: f32,
        handle_color: Color,
        border_color: Color,
    ) -> AppResult<()> {
        let handles = [
            Point::new(rect.x, rect.y),                                          // Top-left
            Point::new(rect.x + rect.width / 2.0, rect.y),                      // Top-center
            Point::new(rect.x + rect.width, rect.y),                            // Top-right
            Point::new(rect.x + rect.width, rect.y + rect.height / 2.0),        // Middle-right
            Point::new(rect.x + rect.width, rect.y + rect.height),              // Bottom-right
            Point::new(rect.x + rect.width / 2.0, rect.y + rect.height),        // Bottom-center
            Point::new(rect.x, rect.y + rect.height),                           // Bottom-left
            Point::new(rect.x, rect.y + rect.height / 2.0),                     // Middle-left
        ];
        
        for handle_center in handles.iter() {
            let handle_rect = Rectangle::new(
                handle_center.x - handle_size / 2.0,
                handle_center.y - handle_size / 2.0,
                handle_size,
                handle_size,
            );
            
            // Draw handle with fill and border
            self.draw_rectangle(
                handle_rect,
                &DrawStyle {
                    fill_color: Some(handle_color),
                    stroke_color: Some(border_color),
                    stroke_width: 1.0,
                    dash_pattern: None,
                },
            )?;
        }
        
        Ok(())
    }
}

/// Platform-specific renderer that combines all traits
pub trait PlatformRenderer: UIRenderer + Send + Sync {
    /// Get a reference to the underlying platform-specific type
    fn as_any(&self) -> &dyn Any;
    
    /// Get a mutable reference to the underlying platform-specific type
    fn as_any_mut(&mut self) -> &mut dyn Any;
    
    /// Create a bitmap from GDI resources (Windows-specific, but abstracted)
    fn create_bitmap_from_gdi(
        &mut self,
        dc: windows::Win32::Graphics::Gdi::HDC,
        width: i32,
        height: i32,
    ) -> AppResult<()>;
}
