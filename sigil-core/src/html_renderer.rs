/*
    Sigil - dynamic image synthesis engine
    Copyright (C) 2025 meetzli

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
*/

use crate::{Sigil, Item};
use std::collections::HashMap;

use dioxus::prelude::*;

pub fn render_to_rsx(sigil: &Sigil, variables: &HashMap<String, String>) -> Element {
    let resolved = sigil.resolve(variables);
    
    let background_style = if resolved.background.starts_with('#') {
        format!("background-color: {}", resolved.background)
    } else if resolved.background.starts_with("http") || resolved.background.starts_with('/') {
        format!("background-image: url('{}'); background-size: cover; background-position: center", resolved.background)
    } else {
        format!("background: {}", resolved.background)
    };
    
    let container_style = format!(
        "position: relative; width: {}px; height: {}px; {}; overflow: hidden;",
        resolved.width, resolved.height, background_style
    );
    
    rsx! {
        div {
            class: "sigil-container",
            style: "{container_style}",
            for layer in resolved.layers.iter() {
                if layer.visible {
                    {
                        let transform = if layer.rotation != 0.0 {
                            format!("rotate({}deg)", layer.rotation)
                        } else {
                            String::new()
                        };
                        
                        rsx! {
                            {match &layer.item {
                                Item::Text(text) => {
                                    let style = format!(
                                        "position: absolute; left: {}px; top: {}px; font-size: {}px; color: {}; font-family: {}; transform: {}; white-space: nowrap;",
                                        layer.x, layer.y, text.font_size, text.color, text.font_family, transform
                                    );
                                    rsx! {
                                        div { style: "{style}", "{text.text}" }
                                    }
                                }
                                Item::Image(img) => {
                                    let border_radius = if img.border_radius > 0.0 {
                                        format!("border-radius: {}px;", img.border_radius)
                                    } else {
                                        String::new()
                                    };
                                    let style = format!(
                                        "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; {} transform: {}; object-fit: cover;",
                                        layer.x, layer.y, img.width, img.height, border_radius, transform
                                    );
                                    rsx! {
                                        img { src: "{img.source}", style: "{style}" }
                                    }
                                }
                                Item::Rect(rect) => {
                                    let border_radius = if rect.border_radius > 0.0 {
                                        format!("border-radius: {}px;", rect.border_radius)
                                    } else {
                                        String::new()
                                    };
                                    let style = format!(
                                        "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; background-color: {}; {} transform: {};",
                                        layer.x, layer.y, rect.width, rect.height, rect.color, border_radius, transform
                                    );
                                    rsx! {
                                        div { style: "{style}" }
                                    }
                                }
                                Item::Slider(slider) => {
                                    let border_radius = if slider.border_radius > 0.0 {
                                        format!("border-radius: {}px;", slider.border_radius)
                                    } else {
                                        String::new()
                                    };
                                    let bg_style = format!(
                                        "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; background-color: {}; {} transform: {};",
                                        layer.x, layer.y, slider.width, slider.height, slider.background_color, border_radius, transform
                                    );
                                    let fill_width = (slider.value / slider.max_value.max(1.0)) * slider.width;
                                    let fill_style = format!(
                                        "position: absolute; left: {}px; top: {}px; width: {}px; height: {}px; background-color: {}; {} transform: {};",
                                        layer.x, layer.y, fill_width, slider.height, slider.fill_color, border_radius, transform
                                    );
                                    rsx! {
                                        div { style: "{bg_style}" }
                                        div { style: "{fill_style}" }
                                    }
                                }
                            }}
                        }
                    }
                }
            }
        }
    }
}

#[cfg(all(test, feature = "rsx"))]
mod tests {
    use super::*;
    use crate::{Layer, TextItem, ImageItem, RectItem};

    #[test]
    fn test_render_to_rsx() {
        let sigil = Sigil {
            width: 800,
            height: 200,
            background: "#18181b".to_string(),
            layers: vec![
                Layer {
                    id: "avatar".to_string(),
                    x: 30.0,
                    y: 30.0,
                    rotation: 0.0,
                    visible: true,
                    item: Item::Image(ImageItem {
                        source: "{avatar}".to_string(),
                        width: 100.0,
                        height: 100.0,
                        border_radius: 9999.0,
                    }),
                },
                Layer {
                    id: "username".to_string(),
                    x: 150.0,
                    y: 50.0,
                    rotation: 0.0,
                    visible: true,
                    item: Item::Text(TextItem {
                        text: "{username}".to_string(),
                        font_size: 32.0,
                        color: "#ffffff".to_string(),
                        font_family: "Sans Serif".to_string(),
                    }),
                },
            ],
        };

        let mut vars = HashMap::new();
        vars.insert("username".to_string(), "TestUser".to_string());
        vars.insert("avatar".to_string(), "https://example.com/avatar.png".to_string());

        let _element = render_to_rsx(&sigil, &vars);
    }
}
