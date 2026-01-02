/*
    Sigil - dynamic image synthesis engine
    Copyright (C) 2025 meetzli

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
*/


use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use sigil_core::{Item, Sigil};
use thiserror::Error;
use tiny_skia::*;
use std::collections::HashMap;
use image::GenericImageView;

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("Failed to create pixmap: {0}")]
    PixmapCreationError(String),

    #[error("Invalid color format: {0}")]
    InvalidColorFormat(String),

    #[error("Invalid dimensions: {0}")]
    InvalidDimensions(String),

    #[error("Font error: {0}")]
    FontError(String),

    #[error("Image decoding error: {0}")]
    ImageError(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

pub struct Renderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    pixmap_buffer: Option<Pixmap>,
    image_cache: HashMap<String, Pixmap>,
    loaded_fonts: std::collections::HashSet<String>,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
            pixmap_buffer: None,
            image_cache: HashMap::new(),
            loaded_fonts: std::collections::HashSet::new(),
        }
    }

    /// Renders the Sigil to the internal buffer and returns the raw pixel data (Premultiplied RGBA8).
    /// This method reuses the internal buffer to avoid allocation overhead.
    pub fn render_raw(&mut self, sigil: &Sigil, resources: &HashMap<String, Vec<u8>>) -> Result<&[u8], RenderError> {
        // Load fonts from resources
        let mut new_fonts = false;
        for (name, data) in resources {
            if (name.ends_with(".ttf") || name.ends_with(".otf") || name.ends_with(".woff2")) && !self.loaded_fonts.contains(name) {
                self.font_system.db_mut().load_font_data(data.clone());
                self.loaded_fonts.insert(name.clone());
                new_fonts = true;
            }
        }
        
        if new_fonts {
            // Log all loaded font families for debugging
            println!("[sigil] Loaded font families:");
            self.font_system.db().faces().for_each(|face| {
                for (name, _) in &face.families {
                    println!("[sigil]   - {}", name);
                }
            });

            let mut first_family = None;
            self.font_system.db().faces().for_each(|face| {
                if first_family.is_none() {
                    if let Some((name, _)) = face.families.first() {
                        first_family = Some(name.clone());
                    }
                }
            });

            if let Some(family) = first_family {
                println!("[sigil] Setting default family to: {}", family);
                let db = self.font_system.db_mut();
                db.set_sans_serif_family(family.clone());
                db.set_serif_family(family.clone());
                db.set_monospace_family(family.clone());
                db.set_cursive_family(family.clone());
                db.set_fantasy_family(family);
            }
        }

        if self.pixmap_buffer.as_ref().map_or(true, |p| p.width() != sigil.width || p.height() != sigil.height) {
            self.pixmap_buffer = Pixmap::new(sigil.width, sigil.height);
        }

        let pixmap = self.pixmap_buffer.as_mut()
            .ok_or_else(|| RenderError::PixmapCreationError("Invalid canvas dimensions".into()))?;

        if let Some(color) = parse_color(&sigil.background) {
            pixmap.fill(color);
        } else {
            let bg_cache_key = format!("bg_{}_{}_{}", sigil.background, sigil.width, sigil.height);
            let bg_pixmap = if let Some(cached) = self.image_cache.get(&bg_cache_key) {
                Some(cached)
            } else if let Some(image_bytes) = resources.get(&sigil.background) {
                if let Ok(dynamic_image) = image::load_from_memory(image_bytes) {
                    let target_width = sigil.width;
                    let target_height = sigil.height;
                    
                    let resized = dynamic_image.resize_to_fill(
                        target_width,
                        target_height,
                        image::imageops::FilterType::Lanczos3
                    );
                    
                    let rgba_image = resized.to_rgba8();
                    let mut pixels = Vec::with_capacity((target_width * target_height * 4) as usize);

                    for pixel in rgba_image.pixels() {
                        let r = pixel[0];
                        let g = pixel[1];
                        let b = pixel[2];
                        let a = pixel[3];

                        let a_f = a as f32 / 255.0;
                        pixels.push((r as f32 * a_f) as u8);
                        pixels.push((g as f32 * a_f) as u8);
                        pixels.push((b as f32 * a_f) as u8);
                        pixels.push(a);
                    }
                    
                    if let Some(pixmap) = Pixmap::from_vec(pixels, IntSize::from_wh(target_width, target_height).unwrap()) {
                        self.image_cache.insert(bg_cache_key.clone(), pixmap);
                        self.image_cache.get(&bg_cache_key)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(bg_pixmap) = bg_pixmap {
                pixmap.draw_pixmap(
                    0, 0,
                    bg_pixmap.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            } else {
                // Fallback to black if neither color nor resource found
                pixmap.fill(Color::BLACK);
            }
        }

        for layer in &sigil.layers {
            let (w, h) = match &layer.item {
                Item::Rect(r) => (r.width, r.height),
                Item::Image(i) => (i.width, i.height),
                Item::Text(_) => (0.0, 0.0),
                Item::Slider(s) => (s.width, s.height),
            };

            let cx = w / 2.0;
            let cy = h / 2.0;

            let layer_transform = Transform::identity()
                .post_translate(-cx, -cy)
                .post_rotate(layer.rotation)
                .post_translate(cx + layer.x, cy + layer.y);

            match &layer.item {
                Item::Rect(rect) => {
                    let color = parse_color(&rect.color)
                        .ok_or_else(|| RenderError::InvalidColorFormat(rect.color.clone()))?;

                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;

                    let r = Rect::from_xywh(0.0, 0.0, rect.width, rect.height)
                        .ok_or_else(|| {
                            RenderError::InvalidDimensions("Rect width/height must be > 0".into())
                        })?;

                    if rect.border_radius > 0.0 {
                        let path = create_rounded_rect_path(r, rect.border_radius);
                        if let Some(p) = path {
                            pixmap.fill_path(
                                &p,
                                &paint,
                                FillRule::Winding,
                                layer_transform,
                                None,
                            );
                        }
                    } else {
                        pixmap.fill_rect(r, &paint, layer_transform, None);
                    }
                }
                Item::Text(text_item) => {
                    let text_color = parse_color(&text_item.color).ok_or_else(|| {
                        RenderError::InvalidColorFormat(text_item.color.clone())
                    })?;

                    let metrics = Metrics::new(text_item.font_size, text_item.font_size * 1.2);
                    let mut buffer = Buffer::new(&mut self.font_system, metrics);

                    let mut attrs = Attrs::new();

                    let family_list: Vec<&str> = text_item.font_family.split(',').map(|s| s.trim()).collect();
                    let mut family = Family::SansSerif;

                    for f in family_list {
                        match f.to_lowercase().as_str() {
                            "arial" | "sans-serif" | "sans serif" | "system-ui" | "-apple-system" => {
                                family = Family::SansSerif;
                                break;
                            }
                            "serif" => {
                                family = Family::Serif;
                                break;
                            }
                            "mono" | "monospace" => {
                                family = Family::Monospace;
                                break;
                            }
                            _ => {
                                // Check if font exists in system
                                // Normalize font name by removing spaces for comparison
                                let normalized_query = f.to_lowercase().replace(' ', "");
                                let mut found_name: Option<String> = None;
                                
                                self.font_system.db().faces().for_each(|face| {
                                    for (name, _) in &face.families {
                                        let normalized_name = name.to_lowercase().replace(' ', "");
                                        if normalized_name == normalized_query || name.to_lowercase() == f.to_lowercase() {
                                            found_name = Some(name.clone());
                                        }
                                    }
                                });

                                if let Some(ref name) = found_name {
                                    println!("[sigil] Matched font '{}' -> '{}'", f, name);
                                    family = Family::Name(Box::leak(name.clone().into_boxed_str()));
                                    break;
                                }
                            }
                        }
                    }

                    // Log if we're using fallback
                    match family {
                        Family::SansSerif => println!("[sigil] Using SansSerif fallback for font_family: {}", text_item.font_family),
                        _ => {}
                    }

                    attrs = attrs.family(family);

                    buffer.set_text(
                        &mut self.font_system,
                        &text_item.text,
                        &attrs,
                        Shaping::Advanced,
                        None,
                    );

                    buffer.shape_until_scroll(&mut self.font_system, false);

                    let mut glyphs_drawn = 0;

                    for run in buffer.layout_runs() {
                        for glyph in run.glyphs {
                            let physical_glyph = glyph.physical((0., 0.), 1.0);

                            if let Some(image) =
                                self.swash_cache.get_image(&mut self.font_system, physical_glyph.cache_key)
                            {
                                let width = image.placement.width;
                                let height = image.placement.height;

                                if width == 0 || height == 0 {
                                    continue;
                                }

                                let glyph_x = (physical_glyph.x as f32) + (image.placement.left as f32);
                                let glyph_y = run.line_y + (physical_glyph.y as f32) - (image.placement.top as f32);

                                let size = IntSize::from_wh(width, height).unwrap();
                                
                                let mut pixels = Vec::with_capacity((width * height * 4) as usize);
                                
                                if image.data.len() == (width * height) as usize {
                                    let r_f = text_color.red();
                                    let g_f = text_color.green();
                                    let b_f = text_color.blue();
                                    let a_f = text_color.alpha();

                                    for mask_val in image.data.iter() {
                                        let mask_alpha = *mask_val as f32 / 255.0;
                                        let final_alpha = a_f * mask_alpha;
                                        
                                        pixels.push((r_f * final_alpha * 255.0) as u8);
                                        pixels.push((g_f * final_alpha * 255.0) as u8);
                                        pixels.push((b_f * final_alpha * 255.0) as u8);
                                        pixels.push((final_alpha * 255.0) as u8);
                                    }
                                } else if image.data.len() == (width * height * 4) as usize {
                                    for chunk in image.data.chunks(4) {
                                        let r = chunk[0];
                                        let g = chunk[1];
                                        let b = chunk[2];
                                        let a = chunk[3];
                                        
                                        let a_f = a as f32 / 255.0;
                                        pixels.push((r as f32 * a_f) as u8);
                                        pixels.push((g as f32 * a_f) as u8);
                                        pixels.push((b as f32 * a_f) as u8);
                                        pixels.push(a);
                                    }
                                } else {
                                    println!("Unknown image format from swash. Length: {}", image.data.len());
                                    continue;
                                }

                                if let Some(glyph_pixmap) = Pixmap::from_vec(pixels, size) {
                                    let glyph_transform = layer_transform
                                        .pre_translate(glyph_x, glyph_y);

                                    pixmap.draw_pixmap(
                                        0, 0,
                                        glyph_pixmap.as_ref(),
                                        &PixmapPaint::default(),
                                        glyph_transform,
                                        None,
                                    );
                                    glyphs_drawn += 1;
                                }
                            } else {
                                println!("Failed to get image from cache for a glyph!");
                            }
                        }
                    }
                }


                Item::Image(img) => {
                    let cache_key = format!("{}_{}_{}", img.source, img.width, img.height);
                    
                    let image_pixmap = if let Some(cached) = self.image_cache.get(&cache_key) {
                        Some(cached)
                    } else if let Some(image_bytes) = resources.get(&img.source) {
                        let dynamic_image = match image::load_from_memory(image_bytes) {
                            Ok(img) => img,
                            Err(e) => {
                                println!("Failed to decode image: {}", e);
                                continue;
                            }
                        };

                        let target_width = img.width as u32;
                        let target_height = img.height as u32;

                        if target_width == 0 || target_height == 0 {
                             println!("Skipping image with 0 dimensions");
                             continue;
                        }

                        let resized = dynamic_image.resize_exact(
                            target_width,
                            target_height,
                            image::imageops::FilterType::Lanczos3
                        );

                        let rgba_image = resized.to_rgba8();
                        let mut pixels = Vec::with_capacity((target_width * target_height * 4) as usize);

                        for pixel in rgba_image.pixels() {
                            let r = pixel[0];
                            let g = pixel[1];
                            let b = pixel[2];
                            let a = pixel[3];

                            let a_f = a as f32 / 255.0;
                            pixels.push((r as f32 * a_f) as u8);
                            pixels.push((g as f32 * a_f) as u8);
                            pixels.push((b as f32 * a_f) as u8);
                            pixels.push(a);
                        }

                        if let Some(pixmap) = Pixmap::from_vec(pixels, IntSize::from_wh(target_width, target_height).unwrap()) {
                            self.image_cache.insert(cache_key.clone(), pixmap);
                            self.image_cache.get(&cache_key)
                        } else {
                            None
                        }
                    } else {
                        println!("Warning: Resource '{}' not found", img.source);
                        None
                    };

                    if let Some(image_pixmap) = image_pixmap {
                        let pattern = Pattern::new(
                            image_pixmap.as_ref(),
                            SpreadMode::Pad,
                            FilterQuality::Bilinear,
                            1.0,
                            Transform::identity(),
                        );

                            let mut paint = Paint::default();
                            paint.shader = pattern;
                            paint.anti_alias = true;

                            let draw_rect = Rect::from_xywh(0.0, 0.0, img.width, img.height).unwrap();

                            let path = if img.border_radius > 0.0 {
                                create_rounded_rect_path(draw_rect, img.border_radius)
                            } else {
                                let mut pb = PathBuilder::new();
                                pb.push_rect(draw_rect);
                                pb.finish()
                            };

                            if let Some(p) = path {
                                pixmap.fill_path(
                                    &p,
                                    &paint,
                                    FillRule::Winding,
                                    layer_transform,
                                    None,
                                );
                            }
                        }
                }
                Item::Slider(slider) => {
                    let bg_color = parse_color(&slider.background_color)
                        .ok_or_else(|| RenderError::InvalidColorFormat(slider.background_color.clone()))?;
                    let fill_color = parse_color(&slider.fill_color)
                        .ok_or_else(|| RenderError::InvalidColorFormat(slider.fill_color.clone()))?;

                    let mut bg_paint = Paint::default();
                    bg_paint.set_color(bg_color);
                    bg_paint.anti_alias = true;

                    let bg_rect = Rect::from_xywh(0.0, 0.0, slider.width, slider.height)
                        .ok_or_else(|| RenderError::InvalidDimensions("Slider width/height must be > 0".into()))?;

                    if slider.border_radius > 0.0 {
                        let path = create_rounded_rect_path(bg_rect, slider.border_radius);
                        if let Some(p) = path {
                            pixmap.fill_path(&p, &bg_paint, FillRule::Winding, layer_transform, None);
                        }
                    } else {
                        pixmap.fill_rect(bg_rect, &bg_paint, layer_transform, None);
                    }

                    let fill_width = (slider.value / slider.max_value.max(1.0)) * slider.width;
                    if fill_width > 0.0 {
                        let mut fill_paint = Paint::default();
                        fill_paint.set_color(fill_color);
                        fill_paint.anti_alias = true;

                        let fill_rect = Rect::from_xywh(0.0, 0.0, fill_width, slider.height)
                            .ok_or_else(|| RenderError::InvalidDimensions("Fill width/height must be > 0".into()))?;

                        if slider.border_radius > 0.0 {
                            let path = create_rounded_rect_path(fill_rect, slider.border_radius);
                            if let Some(p) = path {
                                pixmap.fill_path(&p, &fill_paint, FillRule::Winding, layer_transform, None);
                            }
                        } else {
                            pixmap.fill_rect(fill_rect, &fill_paint, layer_transform, None);
                        }
                    }
                }
            }
        }

        Ok(self.pixmap_buffer.as_ref().unwrap().data())
    }

    pub fn render(&mut self, sigil: &Sigil, resources: &HashMap<String, Vec<u8>>) -> Result<Vec<u8>, RenderError> {
        self.render_raw(sigil, resources)?;
        
        self.pixmap_buffer.as_ref().unwrap()
            .encode_png()
            .map_err(|e| RenderError::EncodingError(e.to_string()))
    }
}

fn create_rounded_rect_path(rect: Rect, radius: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();

    let x = rect.x();
    let y = rect.y();
    let w = rect.width();
    let h = rect.height();

    let r = radius.min(w / 2.0).min(h / 2.0);

    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();

    pb.finish()
}

fn parse_color(hex: &str) -> Option<Color> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return None;
    }

    let r = u8::from_str_radix(&hex[1..3], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..5], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..7], 16).ok()?;

    Some(Color::from_rgba8(r, g, b, 255))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_core::{Layer, RectItem, TextItem};

    #[allow(unused_imports)]
    use std::fs::File;
    #[allow(unused_imports)]
    use std::io::Write;

    #[test]
    fn test_render_rect_and_text() {
        let sigil = Sigil {
            width: 400,
            height: 200,
            background: "#1a1a1a".to_string(),
            layers: vec![
                Layer {
                    id: "box".to_string(),
                    x: 20.0,
                    y: 20.0,
                    rotation: 0.0,
                    item: Item::Rect(RectItem {
                        width: 360.0,
                        height: 160.0,
                        color: "#333333".to_string(),
                        border_radius: 20.0,
                    }),
                },
                Layer {
                    id: "hello".to_string(),
                    x: 50.0,
                    y: 80.0,
                    rotation: 0.0,
                    item: Item::Text(TextItem {
                        text: "Hello Sigil!".to_string(),
                        font_size: 48.0,
                        color: "#ff00ff".to_string(),
                        font_family: "Arial".to_string(),
                    }),
                },
            ],
        };

        let resources = HashMap::new();
        let mut renderer = Renderer::new();
        let png_bytes = renderer.render(&sigil, &resources).expect("Render failed");
        assert!(!png_bytes.is_empty());

        // let mut file = File::create("test_output_text.png").unwrap();
        // file.write_all(&png_bytes).unwrap();
    }
}
