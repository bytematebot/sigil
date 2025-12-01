use sigil_core::{ImageItem, Item, Layer, RectItem, Sigil, TextItem};
use sigil_render::Renderer;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use image::{Rgba, RgbaImage};

fn main() {
    println!("Generating test resources...");

    let mut avatar_img = RgbaImage::new(100, 100);
    for x in 0..100 {
        for y in 0..100 {
            let color = if (x / 10 + y / 10) % 2 == 0 {
                Rgba([255, 0, 0, 255]) // Red
            } else {
                Rgba([0, 0, 255, 255]) // Blue
            };
            avatar_img.put_pixel(x, y, color);
        }
    }

    let mut avatar_bytes = Vec::new();
    avatar_img.write_to(&mut std::io::Cursor::new(&mut avatar_bytes), image::ImageOutputFormat::Png).unwrap();
    println!("Avatar created: {} bytes", avatar_bytes.len());

    let mut resources = HashMap::new();
    resources.insert("{avatar}".to_string(), avatar_bytes);

    let sigil = Sigil {
        width: 400,
        height: 200,
        background: "#222222".to_string(),
        layers: vec![
            Layer {
                id: "card_bg".to_string(),
                x: 10.0,
                y: 10.0,
                rotation: 0.0,
                item: Item::Rect(RectItem {
                    width: 380.0,
                    height: 180.0,
                    color: "#333333".to_string(),
                    border_radius: 16.0,
                }),
            },
            Layer {
                id: "user_avatar".to_string(),
                x: 30.0,
                y: 50.0,
                rotation: 0.0,
                item: Item::Image(ImageItem {
                    source: "{avatar}".to_string(),
                    width: 100.0,
                    height: 100.0,
                    border_radius: 50.0, // Full circle
                }),
            },
            Layer {
                id: "username".to_string(),
                x: 150.0,
                y: 85.0,
                rotation: 0.0,
                item: Item::Text(TextItem {
                    text: "Test User".to_string(),
                    font_size: 32.0,
                    color: "#ffffff".to_string(),
                    font_family: "Sans Serif".to_string(),
                }),
            },
            Layer {
                id: "status".to_string(),
                x: 150.0,
                y: 120.0,
                rotation: 0.0,
                item: Item::Text(TextItem {
                    text: "Level 42 Paladin".to_string(),
                    font_size: 18.0,
                    color: "#aaaaaa".to_string(),
                    font_family: "Sans Serif".to_string(),
                }),
            },
        ],
    };

    println!("Initializing Renderer...");
    let mut renderer = Renderer::new();
    
    println!("Warmup render...");
    let start = std::time::Instant::now();
    let _ = renderer.render(&sigil, &resources).expect("Failed to render");
    println!("Warmup finished in {:?}", start.elapsed());

    let iterations = 100;
    println!("Starting stress test ({} iterations)...", iterations);
    let start_stress = std::time::Instant::now();
    
    for _ in 0..iterations {
        let _ = renderer.render_raw(&sigil, &resources).expect("Failed to render");
    }
    
    let total_duration = start_stress.elapsed();
    let avg_duration = total_duration / iterations;
    
    println!("Stress test finished in {:?}", total_duration);
    println!("Average render time: {:?}", avg_duration);

    let output_bytes = renderer.render(&sigil, &resources).expect("Failed to render");
    let mut file = File::create("avatar_test.png").unwrap();
    file.write_all(&output_bytes).unwrap();
    println!("Saved to avatar_test.png");
}
