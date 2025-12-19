/*
    Sigil - dynamic image synthesis engine
    Copyright (C) 2025 meetzli

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
*/


use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[cfg(feature = "rsx")]
pub mod html_renderer;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sigil {
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub layers: Vec<Layer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub id: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
    pub item: Item,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum Item {
    Text(TextItem),
    Image(ImageItem),
    Rect(RectItem),
    Slider(SliderItem),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextItem {
    pub text: String,
    pub font_size: f32,
    pub color: String,
    pub font_family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageItem {
    pub source: String,
    pub width: f32,
    pub height: f32,
    pub border_radius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RectItem {
    pub width: f32,
    pub height: f32,
    pub color: String,
    pub border_radius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SliderItem {
    pub width: f32,
    pub height: f32,
    pub value: f32,
    pub max_value: f32,
    pub background_color: String,
    pub fill_color: String,
    pub border_radius: f32,
}


impl Sigil {
    pub fn resolve(&self, variables: &HashMap<String, String>) -> Self {
        let mut new_sigil = self.clone();

        new_sigil.background = replace_vars(&new_sigil.background, variables);

        for layer in &mut new_sigil.layers {
            match &mut layer.item {
                Item::Text(text) => {
                    text.text = replace_vars(&text.text, variables);
                    text.color = replace_vars(&text.color, variables);
                },
                Item::Image(img) => {
                    img.source = replace_vars(&img.source, variables);
                },
                Item::Rect(rect) => {
                    rect.color = replace_vars(&rect.color, variables);
                },
                Item::Slider(slider) => {
                    slider.background_color = replace_vars(&slider.background_color, variables);
                    slider.fill_color = replace_vars(&slider.fill_color, variables);
                }
            }
        }
        new_sigil
    }
}

fn replace_vars(input: &str, vars: &HashMap<String, String>) -> String {
    let mut result = input.to_string();
    for (k, v) in vars {
        let placeholder = format!("{{{}}}", k);
        result = result.replace(&placeholder, v);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_serializes_correctly() {
        let sigil = Sigil {
            width: 800,
            height: 400,
            background: "#1a1a1a".to_string(),
            layers: vec![
                Layer {
                    id: "avatar_layer".to_string(),
                    x: 50.0,
                    y: 50.0,
                    rotation: 0.0,
                    item: Item::Image(ImageItem {
                        source: "{avatar}".to_string(),
                        width: 100.0,
                        height: 100.0,
                        border_radius: 50.0,
                    }),
                },
                Layer {
                    id: "welcome_text".to_string(),
                    x: 170.0,
                    y: 100.0,
                    rotation: 0.0,
                    item: Item::Text(TextItem {
                        text: "Welcome {username}!".to_string(),
                        font_size: 48.0,
                        color: "#ffffff".to_string(),
                        font_family: "Roboto".to_string(),
                    }),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&sigil).unwrap();
        println!("{}", json);
    }
}
