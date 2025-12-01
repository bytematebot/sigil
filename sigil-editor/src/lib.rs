/*
    Sigil - dynamic image synthesis engine
    Copyright (C) 2025 meetzli

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
*/


#![allow(non_snake_case)]

use dioxus::prelude::*;
use std::collections::{HashSet, HashMap};
use sigil_core::{Sigil, Layer, Item, RectItem, TextItem, ImageItem};

const MAIN_CSS: Asset = asset!("/assets/editor.css");

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandleType {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub enum DragMode {
    Move {
        start_x: f64,
        start_y: f64,
        original_positions: Vec<(usize, f32, f32)>,
    },
    Resize {
        handle: HandleType,
        start_x: f64,
        start_y: f64,
        orig_x: f32,
        orig_y: f32,
        orig_w: f32,
        orig_h: f32,
    },
    Rotate {
        orig_rotation: f32,
        center_x: f64,
        center_y: f64,
        start_angle: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Guide {
    is_vertical: bool,
    pos: f32,
    start: f32,
    end: f32,
}

const GRID_SIZE: f32 = 20.0;

fn snap_to_grid(val: f32) -> f32 {
    (val / GRID_SIZE).round() * GRID_SIZE
}

#[component]
pub fn SigilEditor() -> Element {
    let mut sigil = use_signal(|| Sigil {
        width: 400,
        height: 200,
        background: "#222222".to_string(),
        layers: vec![
            Layer {
                id: "bg".to_string(),
                x: 0.0,
                y: 0.0,
                rotation: 0.0,
                visible: true,
                item: Item::Rect(RectItem {
                    width: 400.0,
                    height: 200.0,
                    color: "#333333".to_string(),
                    border_radius: 0.0,
                }),
            },
            Layer {
                id: "text".to_string(),
                x: 20.0,
                y: 50.0,
                rotation: 0.0,
                visible: true,
                item: Item::Text(TextItem {
                    text: "Hello Dioxus!".to_string(),
                    font_size: 32.0,
                    color: "#ffffff".to_string(),
                    font_family: "Sans Serif".to_string(),
                }),
            }
        ],
    });

    let mut dragging = use_signal(|| None::<(usize, DragMode)>);
    let mut dragging_layer_index = use_signal(|| None::<usize>);
    let mut drag_over_state = use_signal(|| None::<(usize, bool)>);
    let mut selected_layers = use_signal(|| HashSet::<usize>::new());
    let mut locked_layers = use_signal(|| HashSet::<usize>::new());
    let mut clipboard = use_signal(|| Vec::<Layer>::new());
    let mut guides = use_signal(|| Vec::<Guide>::new());
    let mut text_dimensions = use_signal(|| HashMap::<String, (f32, f32)>::new());
    let mut add_layer_type = use_signal(|| "Rectangle".to_string());
    let mut layer_id_counter = use_signal(|| 2);
    let mut show_load_modal = use_signal(|| false);
    let mut load_json_text = use_signal(|| String::new());
    let mut load_error = use_signal(|| None::<String>);

    let cursor_style = if dragging.read().is_some() { "grabbing" } else { "default" };

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div {
            class: "editor-container",
            style: "cursor: {cursor_style};",
            tabindex: "0",
            onkeydown: move |evt| {
                let is_ctrl = evt.modifiers().contains(Modifiers::CONTROL) || evt.modifiers().contains(Modifiers::META);

                if evt.key() == Key::Character("a".to_string()) && is_ctrl {
                    let len = sigil.read().layers.len();
                    let all_indices: HashSet<usize> = (0..len).collect();
                    selected_layers.set(all_indices);
                    evt.stop_propagation();
                    evt.prevent_default();
                }

                if evt.key() == Key::Character("c".to_string()) && is_ctrl {
                    let selected = selected_layers.read();
                    let layers = &sigil.read().layers;
                    let mut to_copy = Vec::new();
                    for &idx in selected.iter() {
                        if let Some(layer) = layers.get(idx) {
                            to_copy.push(layer.clone());
                        }
                    }
                    if !to_copy.is_empty() {
                        clipboard.set(to_copy);
                    }
                    evt.stop_propagation();
                    evt.prevent_default();
                }

                if evt.key() == Key::Character("v".to_string()) && is_ctrl {
                    let to_paste = clipboard.read().clone();
                    if !to_paste.is_empty() {
                        let mut new_selection = HashSet::new();
                        let mut current_id = *layer_id_counter.read();
                        
                        for mut layer in to_paste {
                            current_id += 1;
                            layer.id = format!("{}_{}", layer.id, current_id);
                            layer.x += 20.0;
                            layer.y += 20.0;
                            
                            sigil.write().layers.push(layer);
                            new_selection.insert(sigil.read().layers.len() - 1);
                        }
                        
                        layer_id_counter.set(current_id);
                        selected_layers.set(new_selection);
                    }
                    evt.stop_propagation();
                    evt.prevent_default();
                }

                if evt.key() == Key::Delete {
                    let to_remove: Vec<usize> = selected_layers.read().iter().cloned().collect();
                    if !to_remove.is_empty() {
                        let mut sorted = to_remove;
                        sorted.sort_by(|a, b| b.cmp(a));
                        for idx in sorted {
                            sigil.write().layers.remove(idx);
                        }
                        selected_layers.write().clear();
                    }
                }

                if evt.key() == Key::Character("o".to_string()) && is_ctrl {
                    load_json_text.set(String::new());
                    load_error.set(None);
                    show_load_modal.set(true);
                    evt.stop_propagation();
                    evt.prevent_default();
                }

                if evt.key() == Key::Escape {
                    if *show_load_modal.read() {
                        show_load_modal.set(false);
                        evt.stop_propagation();
                        evt.prevent_default();
                    }
                }
            },
            onmousemove: move |evt| {
                let mut dragging_write = dragging.write();
                if let Some((drag_idx, ref mut mode)) = *dragging_write {
                    let coords = evt.page_coordinates();
                    
                    match mode {
                        DragMode::Move { start_x, start_y, original_positions } => {
                            let mut delta_x = coords.x - *start_x;
                            let mut delta_y = coords.y - *start_y;
                            
                            let mut lock_x = false;
                            let mut lock_y = false;

                            if evt.modifiers().contains(Modifiers::SHIFT) {
                                if delta_x.abs() > delta_y.abs() {
                                    delta_y = 0.0;
                                    lock_y = true;
                                } else {
                                    delta_x = 0.0;
                                    lock_x = true;
                                }
                            }

                            guides.write().clear();
                            
                            if let Some((_, orig_x, orig_y)) = original_positions.iter().find(|(idx, _, _)| *idx == drag_idx) {
                                let sigil_read = sigil.read();
                                let canvas_w = sigil_read.width as f32;
                                let canvas_h = sigil_read.height as f32;
                                
                                if let Some(layer) = sigil_read.layers.get(drag_idx) {
                                    let (w, h) = match &layer.item {
                                        Item::Rect(r) => (r.width, r.height),
                                        Item::Image(i) => (i.width, i.height),
                                        Item::Text(t) => {
                                            if let Some(&(tw, th)) = text_dimensions.read().get(&layer.id) {
                                                (tw, th)
                                            } else {
                                                (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
                                            }
                                        },
                                    };

                                    let mut proposed_x = *orig_x + delta_x as f32;
                                    let mut proposed_y = *orig_y + delta_y as f32;
                                    
                                    let threshold = 5.0;
                                    let mut snap_x_delta: Option<f32> = None;
                                    let mut snap_y_delta: Option<f32> = None;

                                    let mut check_snap = |val: f32, target: f32, is_vertical: bool, start: f32, end: f32| {
                                        let diff = target - val;
                                        if diff.abs() < threshold {
                                            if is_vertical {
                                                if snap_x_delta.is_none() || diff.abs() < snap_x_delta.unwrap().abs() {
                                                    snap_x_delta = Some(diff);
                                            
                                                }
                                            } else {
                                                if snap_y_delta.is_none() || diff.abs() < snap_y_delta.unwrap().abs() {
                                                    snap_y_delta = Some(diff);
                                                }
                                            }
                                            return true;
                                        }
                                        false
                                    };


                                    let v_targets = vec![
                                        (0.0, 0.0, canvas_h), 
                                        (canvas_w / 2.0, 0.0, canvas_h), 
                                        (canvas_w, 0.0, canvas_h), 
                                    ];
                                    
                                    let h_targets = vec![
                                        (0.0, 0.0, canvas_w), 
                                        (canvas_h / 2.0, 0.0, canvas_w), 
                                        (canvas_h, 0.0, canvas_w),
                                    ];

                                    let mut other_v_targets = Vec::new();
                                    let mut other_h_targets = Vec::new();
                                    
                                    for (i, l) in sigil_read.layers.iter().enumerate() {
                                        if i != drag_idx && !selected_layers.read().contains(&i) && l.visible {
                                            let (lw, lh) = match &l.item {
                                                Item::Rect(r) => (r.width, r.height),
                                                Item::Image(img) => (img.width, img.height),
                                                Item::Text(t) => {
                                                    if let Some(&(tw, th)) = text_dimensions.read().get(&l.id) {
                                                        (tw, th)
                                                    } else {
                                                        (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
                                                    }
                                                },
                                            };
                                            
                                            other_v_targets.push((l.x, l.y, l.y + lh)); 
                                            other_v_targets.push((l.x + lw / 2.0, l.y, l.y + lh)); 
                                            other_v_targets.push((l.x + lw, l.y, l.y + lh)); 
                                            
                                            other_h_targets.push((l.y, l.x, l.x + lw)); 
                                            other_h_targets.push((l.y + lh / 2.0, l.x, l.x + lw)); 
                                            other_h_targets.push((l.y + lh, l.x, l.x + lw)); 
                                        }
                                    }

                                    let mut best_v_guide = None;
                                    
                                    if !lock_x {
                                        let x_points = vec![
                                            (proposed_x, 0.0), 
                                            (proposed_x + w / 2.0, w / 2.0), 
                                            (proposed_x + w, w), 
                                        ];

                                        for (pt_x, offset) in x_points {
                                            for &(target, t_start, t_end) in v_targets.iter().chain(other_v_targets.iter()) {
                                                if (pt_x - target).abs() < threshold {
                                                    if snap_x_delta.is_none() || (target - pt_x).abs() < snap_x_delta.unwrap().abs() {
                                                        snap_x_delta = Some(target - pt_x);
                                                        let min_y = proposed_y.min(t_start);
                                                        let max_y = (proposed_y + h).max(t_end);
                                                        best_v_guide = Some(Guide { is_vertical: true, pos: target, start: min_y, end: max_y });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    let mut best_h_guide = None;
                                    
                                    if !lock_y {
                                        let y_points = vec![
                                            (proposed_y, 0.0), 
                                            (proposed_y + h / 2.0, h / 2.0), 
                                            (proposed_y + h, h), 
                                        ];

                                        for (pt_y, offset) in y_points {
                                            for &(target, t_start, t_end) in h_targets.iter().chain(other_h_targets.iter()) {
                                                if (pt_y - target).abs() < threshold {
                                                    if snap_y_delta.is_none() || (target - pt_y).abs() < snap_y_delta.unwrap().abs() {
                                                        snap_y_delta = Some(target - pt_y);
                                                        let min_x = proposed_x.min(t_start);
                                                        let max_x = (proposed_x + w).max(t_end);
                                                        best_h_guide = Some(Guide { is_vertical: false, pos: target, start: min_x, end: max_x });
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if let Some(dx) = snap_x_delta {
                                        delta_x += dx as f64;
                                        if let Some(g) = best_v_guide { guides.write().push(g); }
                                    }
                                    if let Some(dy) = snap_y_delta {
                                        delta_y += dy as f64;
                                        if let Some(g) = best_h_guide { guides.write().push(g); }
                                    }
                                }
                            }

                            for (idx, orig_x, orig_y) in original_positions {
                                let mut new_x = *orig_x + delta_x as f32;
                                let mut new_y = *orig_y + delta_y as f32;

                                
                                if let Some(layer) = sigil.write().layers.get_mut(*idx) {
                                    layer.x = new_x;
                                    layer.y = new_y;
                                }
                            }
                        },
                        DragMode::Resize { handle, start_x, start_y, orig_x, orig_y, orig_w, orig_h } => {
                            if let Some(&idx) = selected_layers.read().iter().next() {
                                let delta_x = (coords.x - *start_x) as f32;
                                let delta_y = (coords.y - *start_y) as f32;
                                
                                let mut new_x = *orig_x;
                                let mut new_y = *orig_y;
                                let mut new_w = *orig_w;
                                let mut new_h = *orig_h;
                                
                                match handle {
                                    HandleType::BottomRight => {
                                        new_w = *orig_w + delta_x;
                                        new_h = *orig_h + delta_y;
                                    },
                                    HandleType::BottomLeft => {
                                        new_x = *orig_x + delta_x;
                                        new_w = *orig_w - delta_x;
                                        new_h = *orig_h + delta_y;
                                    },
                                    HandleType::TopRight => {
                                        new_y = *orig_y + delta_y;
                                        new_w = *orig_w + delta_x;
                                        new_h = *orig_h - delta_y;
                                    },
                                    HandleType::TopLeft => {
                                        new_x = *orig_x + delta_x;
                                        new_y = *orig_y + delta_y;
                                        new_w = *orig_w - delta_x;
                                        new_h = *orig_h - delta_y;
                                    },
                                    HandleType::Top => {
                                        new_y = *orig_y + delta_y;
                                        new_h = *orig_h - delta_y;
                                    },
                                    HandleType::Bottom => {
                                        new_h = *orig_h + delta_y;
                                    },
                                    HandleType::Left => {
                                        new_x = *orig_x + delta_x;
                                        new_w = *orig_w - delta_x;
                                    },
                                    HandleType::Right => {
                                        new_w = *orig_w + delta_x;
                                    }
                                }

                                new_x = snap_to_grid(new_x);
                                new_y = snap_to_grid(new_y);
                                new_w = snap_to_grid(new_w);
                                new_h = snap_to_grid(new_h);
                                
                                if new_w < GRID_SIZE { new_w = GRID_SIZE; }
                                if new_h < GRID_SIZE { new_h = GRID_SIZE; }
                                
                                if let Some(layer) = sigil.write().layers.get_mut(idx) {
                                    layer.x = new_x;
                                    layer.y = new_y;
                                    
                                    match &mut layer.item {
                                        Item::Rect(r) => { r.width = new_w; r.height = new_h; },
                                        Item::Image(i) => { i.width = new_w; i.height = new_h; },
                                        _ => {}
                                    }
                                }
                            }
                        },
                        DragMode::Rotate { orig_rotation, center_x, center_y, start_angle } => {
                            if let Some(&idx) = selected_layers.read().iter().next() {
                                let coords = evt.page_coordinates();
                                let current_angle = (coords.y - *center_y).atan2(coords.x - *center_x);
                                
                                let delta_angle = current_angle - *start_angle;
                                let new_rotation = *orig_rotation + delta_angle.to_degrees() as f32;
                                
                                if let Some(layer) = sigil.write().layers.get_mut(idx) {
                                    layer.rotation = new_rotation;
                                }
                            }
                        }
                    }
                }
            },
            onmouseup: move |_| {
                dragging.set(None);
                guides.write().clear();
            },

            div {
                class: "left-panel",
                
                div {
                    class: "header-actions",
                    h2 { "Sigil Editor" }
                    button {
                        class: "primary-btn",
                        r#type: "button",
                        onclick: move |_| async move {
                            let json = serde_json::to_string_pretty(&*sigil.read()).unwrap();
                            let mut eval = document::eval(&format!("navigator.clipboard.writeText(`{}`)", json));
                            let _: Result<serde_json::Value, _> = eval.recv().await;
                        },
                        "Copy JSON"
                    }
                    button {
                        class: "primary-btn",
                        r#type: "button",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            evt.prevent_default();
                            load_json_text.set(String::new());
                            load_error.set(None);
                            show_load_modal.set(true);
                        },
                        "Load JSON"
                    }
                }
                
                div {
                    class: "control-group",
                    label { "Canvas Width" }
                    input {
                        r#type: "number",
                        value: "{sigil.read().width}",
                        oninput: move |evt| {
                            if let Ok(w) = evt.value().parse::<u32>() {
                                sigil.write().width = w;
                            }
                        }
                    }
                }
                div {
                    class: "control-group",
                    label { "Canvas Height" }
                    input {
                        r#type: "number",
                        value: "{sigil.read().height}",
                        oninput: move |evt| {
                            if let Ok(h) = evt.value().parse::<u32>() {
                                sigil.write().height = h;
                            }
                        }
                    }
                }

                    div {
                        class: "inspector-panel",
                        h3 { "Properties" }
                        
                        if selected_layers.read().is_empty() {
                            div { class: "empty-state", style: "color: #888; text-align: center; padding: 20px;", "Select a layer to edit properties" }
                        } else if selected_layers.read().len() > 1 {
                            div { class: "empty-state", style: "color: #888; text-align: center; padding: 20px;", "Multiple layers selected" }
                        } else {
                            if let Some(&idx) = selected_layers.read().iter().next() {
                                if let Some(layer) = sigil.read().layers.get(idx) {
                                    {
                                        let properties = match &layer.item {
                                            Item::Rect(r) => rsx! {
                                                div {
                                                    class: "control-group",
                                                    label { "Width: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{r.width}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Rect(ref mut rect) = sigil.write().layers[idx].item {
                                                                    rect.width = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Height: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{r.height}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Rect(ref mut rect) = sigil.write().layers[idx].item {
                                                                    rect.height = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Color: " }
                                                    input {
                                                        r#type: "color",
                                                        value: "{r.color}",
                                                        oninput: move |evt| {
                                                            if let Item::Rect(ref mut rect) = sigil.write().layers[idx].item {
                                                                rect.color = evt.value();
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Radius: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{r.border_radius}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Rect(ref mut rect) = sigil.write().layers[idx].item {
                                                                    rect.border_radius = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                            Item::Image(i) => rsx! {
                                                div {
                                                    class: "control-group",
                                                    label { "Width: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{i.width}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Image(ref mut img) = sigil.write().layers[idx].item {
                                                                    img.width = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Height: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{i.height}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Image(ref mut img) = sigil.write().layers[idx].item {
                                                                    img.height = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Source: " }
                                                    input {
                                                        r#type: "text",
                                                        value: "{i.source}",
                                                        oninput: move |evt| {
                                                            if let Item::Image(ref mut img) = sigil.write().layers[idx].item {
                                                                img.source = evt.value();
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Radius: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{i.border_radius}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Image(ref mut img) = sigil.write().layers[idx].item {
                                                                    img.border_radius = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            },
                                            Item::Text(t) => rsx! {
                                                div {
                                                    class: "control-group",
                                                    label { "Text: " }
                                                    input {
                                                        r#type: "text",
                                                        value: "{t.text}",
                                                        oninput: move |evt| {
                                                            if let Item::Text(ref mut text) = sigil.write().layers[idx].item {
                                                                text.text = evt.value();
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Font Size: " }
                                                    input {
                                                        r#type: "number",
                                                        value: "{t.font_size}",
                                                        oninput: move |evt| {
                                                            if let Ok(val) = evt.value().parse::<f32>() {
                                                                if let Item::Text(ref mut text) = sigil.write().layers[idx].item {
                                                                    text.font_size = val;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Color: " }
                                                    input {
                                                        r#type: "color",
                                                        value: "{t.color}",
                                                        oninput: move |evt| {
                                                            if let Item::Text(ref mut text) = sigil.write().layers[idx].item {
                                                                text.color = evt.value();
                                                            }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "control-group",
                                                    label { "Font Family: " }
                                                    select {
                                                        value: "{t.font_family}",
                                                        oninput: move |evt| {
                                                            if let Item::Text(ref mut text) = sigil.write().layers[idx].item {
                                                                text.font_family = evt.value();
                                                            }
                                                        },
                                                        option { value: "Sans Serif", style: "font-family: sans-serif;", "Sans Serif" }
                                                        option { value: "Serif", style: "font-family: serif;", "Serif" }
                                                        option { value: "Monospace", style: "font-family: monospace;", "Monospace" }
                                                        option { value: "Cursive", style: "font-family: cursive;", "Cursive" }
                                                        option { value: "Fantasy", style: "font-family: fantasy;", "Fantasy" }
                                                    }
                                                }
                                            }
                                        };

                                        rsx! {
                                            div {
                                                class: "control-group",
                                                label { "X: " }
                                                input {
                                                    r#type: "number",
                                                    value: "{layer.x}",
                                                    oninput: move |evt| {
                                                        if let Ok(val) = evt.value().parse::<f32>() {
                                                            sigil.write().layers[idx].x = val;
                                                        }
                                                    }
                                                }
                                            }
                                            div {
                                                class: "control-group",
                                                label { "Y: " }
                                                input {
                                                    r#type: "number",
                                                    value: "{layer.y}",
                                                    oninput: move |evt| {
                                                        if let Ok(val) = evt.value().parse::<f32>() {
                                                            sigil.write().layers[idx].y = val;
                                                        }
                                                    }
                                                }
                                            }
                                            div {
                                                class: "control-group",
                                                label { "Rotation: " }
                                                input {
                                                    r#type: "number",
                                                    value: "{layer.rotation}",
                                                    oninput: move |evt| {
                                                        if let Ok(val) = evt.value().parse::<f32>() {
                                                            sigil.write().layers[idx].rotation = val;
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            {properties}
                                        }
                                    }
                                }
                            }
                        }
                    }

                h3 { "Layers" }

                div {
                    class: "add-layer-controls",
                    select {
                        value: "{add_layer_type}",
                        oninput: move |evt| add_layer_type.set(evt.value()),
                        option { value: "Rectangle", "Rectangle" }
                        option { value: "Text", "Text" }
                        option { value: "Image", "Image" }
                    }
                    button {
                        class: "primary-btn",
                        onclick: move |_| {
                            let current_id = *layer_id_counter.read();
                            layer_id_counter.set(current_id + 1);
                            
                            let layer_type = add_layer_type.read().clone();
                            let new_layer = match layer_type.as_str() {
                                "Rectangle" => Layer {
                                    id: format!("rect_{}", current_id),
                                    x: 50.0, y: 50.0, rotation: 0.0,
                                    visible: true,
                                    item: Item::Rect(RectItem { width: 100.0, height: 100.0, color: "#cccccc".to_string(), border_radius: 0.0 })
                                },
                                "Text" => Layer {
                                    id: format!("text_{}", current_id),
                                    x: 50.0, y: 50.0, rotation: 0.0,
                                    visible: true,
                                    item: Item::Text(TextItem { text: "New Text".to_string(), font_size: 24.0, color: "#ffffff".to_string(), font_family: "Sans Serif".to_string() })
                                },
                                "Image" => Layer {
                                    id: format!("img_{}", current_id),
                                    x: 50.0, y: 50.0, rotation: 0.0,
                                    visible: true,
                                    item: Item::Image(ImageItem { width: 100.0, height: 100.0, source: "".to_string(), border_radius: 0.0 })
                                },
                                _ => return,
                            };
                            sigil.write().layers.push(new_layer);

                            let new_idx = sigil.read().layers.len() - 1;
                            selected_layers.write().clear();
                            selected_layers.write().insert(new_idx);
                        },
                        "Add"
                    }
                }

                div {
                    class: "layer-actions",
                    button { 
                        class: "action-btn", 
                        title: "Move Up",
                        disabled: selected_layers.read().len() != 1,
                        onclick: move |_| {
                            let idx_opt = selected_layers.read().iter().next().cloned();
                            if let Some(idx) = idx_opt {
                                if idx < sigil.read().layers.len() - 1 {
                                    sigil.write().layers.swap(idx, idx + 1);
                                    selected_layers.write().clear();
                                    selected_layers.write().insert(idx + 1);
                                }
                            }
                        },
                        "Up" 
                    }
                    button { 
                        class: "action-btn", 
                        title: "Move Down",
                        disabled: selected_layers.read().len() != 1,
                        onclick: move |_| {
                            let idx_opt = selected_layers.read().iter().next().cloned();
                            if let Some(idx) = idx_opt {
                                if idx > 0 {
                                    sigil.write().layers.swap(idx, idx - 1);
                                    selected_layers.write().clear();
                                    selected_layers.write().insert(idx - 1);
                                }
                            }
                        },
                        "Down" 
                    }
                    button { 
                        class: "action-btn danger", 
                        title: "Delete Layer",
                        disabled: selected_layers.read().is_empty(),
                        onclick: move |_| {
                            let to_remove: Vec<usize> = selected_layers.read().iter().cloned().collect();
                            if !to_remove.is_empty() {
                                let mut sorted = to_remove;
                                sorted.sort_by(|a, b| b.cmp(a));
                                for idx in sorted {
                                    sigil.write().layers.remove(idx);
                                }
                                selected_layers.write().clear();
                            }
                        },
                        "Del" 
                    }
                }

                div {
                    class: "layers-list",
                    onmouseleave: move |_| {
                        drag_over_state.set(None);
                    },
                    for (idx, layer) in sigil.read().layers.iter().enumerate() {
                        div {
                            key: "{layer.id}",
                            class: if selected_layers.read().contains(&idx) { "layer-item selected" } else { "layer-item" },
                            draggable: true,
                            ondragstart: move |_| {
                                dragging_layer_index.set(Some(idx));
                            },
                            ondragover: move |evt| {
                                evt.prevent_default();
                                let coords = evt.element_coordinates();
                                let is_top = coords.y < 20.0; 
                                drag_over_state.set(Some((idx, is_top)));
                            },
                            ondrop: move |evt| {
                                evt.prevent_default();
                                if let Some(from_idx) = *dragging_layer_index.read() {
                                    if from_idx != idx {
                                        let mut s = sigil.write();
                                        if from_idx < s.layers.len() {
                                            let item = s.layers.remove(from_idx);
                                            let is_top = (*drag_over_state.read()).map(|(_, top)| top).unwrap_or(true);
                                            let mut target_idx = idx;
                                            if from_idx < idx {
                                                target_idx -= 1;
                                            }
                                            
                                            if !is_top {
                                                target_idx += 1;
                                            }
                                            
                                            if target_idx <= s.layers.len() {
                                                s.layers.insert(target_idx, item);
                                                selected_layers.write().clear();
                                                selected_layers.write().insert(target_idx);
                                            }
                                        }
                                    }
                                }
                                dragging_layer_index.set(None);
                                drag_over_state.set(None);
                            },
                            
                            onclick: move |evt| {
                                let is_ctrl = evt.modifiers().contains(Modifiers::CONTROL) || evt.modifiers().contains(Modifiers::META);
                                let is_shift = evt.modifiers().contains(Modifiers::SHIFT);
                                
                                if is_ctrl || is_shift {
                                    if selected_layers.read().contains(&idx) {
                                        selected_layers.write().remove(&idx);
                                    } else {
                                        selected_layers.write().insert(idx);
                                    }
                                } else {
                                    selected_layers.write().clear();
                                    selected_layers.write().insert(idx);
                                }
                            },

                            if let Some((over_idx, is_top)) = *drag_over_state.read() {
                                if over_idx == idx {
                                    div {
                                        class: if is_top { "drop-indicator top" } else { "drop-indicator bottom" }
                                    }
                                }
                            }
                            div {
                                class: "layer-info",
                                div { strong { "{layer.id}" } }
                                div { "Type: {item_type_name(&layer.item)}" }
                            }
                            div {
                                class: "layer-controls",
                                button {
                                    class: "icon-btn",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        let current = sigil.read().layers[idx].visible;
                                        sigil.write().layers[idx].visible = !current;
                                    },
                                    if layer.visible { "👁️" } else { "🚫" }
                                }
                                button {
                                    class: "icon-btn",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        if locked_layers.read().contains(&idx) {
                                            locked_layers.write().remove(&idx);
                                        } else {
                                            locked_layers.write().insert(idx);
                                        }
                                    },
                                    if locked_layers.read().contains(&idx) { "🔒" } else { "🔓" }
                                }
                            }
                        }
                    }
                }
            }

            div {
                class: "right-panel",
                onclick: move |_| {
                    selected_layers.write().clear();
                },
                
                h2 { "Preview (Drag items to move)" }

                div {
                    class: "canvas-container",
                    style: "
                        width: {sigil.read().width}px; 
                        height: {sigil.read().height}px; 
                        background-color: {sigil.read().background};
                        cursor: {cursor_style};
                    ",
                    onclick: move |_| {
                        selected_layers.write().clear();
                    },
                    
                    for (idx, layer) in sigil.read().layers.iter().enumerate() {
                        if layer.visible {
                            {
                                let is_selected = selected_layers.read().contains(&idx);
                                let is_locked = locked_layers.read().contains(&idx);
                                rsx!{
                                    RenderLayer {
                                        key: "{layer.id}",
                                        layer: layer.clone(),
                                        is_selected,
                                        is_locked,
                                        text_dimensions: text_dimensions,
                                        on_move_start: move |evt: MouseEvent| {
                                        if locked_layers.read().contains(&idx) {
                                            return;
                                        }

                                        let is_ctrl = evt.modifiers().contains(Modifiers::CONTROL) || evt.modifiers().contains(Modifiers::META);
                                        let is_shift = evt.modifiers().contains(Modifiers::SHIFT);
                                        
                                        if !selected_layers.read().contains(&idx) {
                                            if !is_ctrl && !is_shift {
                                                selected_layers.write().clear();
                                            }
                                            selected_layers.write().insert(idx);
                                        } else if is_ctrl {
                                        }

                                        let coords = evt.page_coordinates();

                                        let mut original_positions = Vec::new();
                                        for &sel_idx in selected_layers.read().iter() {
                                            if let Some(l) = sigil.read().layers.get(sel_idx) {
                                                if !locked_layers.read().contains(&sel_idx) {
                                                    original_positions.push((sel_idx, l.x, l.y));
                                                }
                                            }
                                        }

                                        if !original_positions.is_empty() {
                                            dragging.set(Some((idx, DragMode::Move {
                                                start_x: coords.x,
                                                start_y: coords.y,
                                                original_positions,
                                            })));
                                        }
                                        evt.stop_propagation();
                                    }
                                }
                            }
                            }
                        }
                    }

                    {
                        let indices: Vec<usize> = selected_layers.read().iter().cloned().collect();
                        indices.into_iter().map(|idx| {
                            if let Some(layer) = sigil.read().layers.get(idx) {
                                if !layer.visible || locked_layers.read().contains(&idx) {
                                    return rsx!({});
                                }

                                let layer_rot = layer.rotation;
                                let layer_x = layer.x;
                                let layer_y = layer.y;
                                rsx! {
                                    SelectionOverlay {
                                        key: "overlay_{idx}",
                                        layer: layer.clone(),
                                        text_dimensions: text_dimensions,
                                        on_resize_start: move |(handle, evt): (HandleType, MouseEvent)| {
                                            if selected_layers.read().len() == 1 {
                                                let coords = evt.page_coordinates();
                                                let (w, h) = match &sigil.read().layers[idx].item {
                                                    Item::Rect(r) => (r.width, r.height),
                                                    Item::Image(i) => (i.width, i.height),
                                                    Item::Text(t) => {
                                                        if let Some(&(tw, th)) = text_dimensions.read().get(&sigil.read().layers[idx].id) {
                                                            (tw, th)
                                                        } else {
                                                            (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
                                                        }
                                                    },
                                                };
                                                dragging.set(Some((idx, DragMode::Resize {
                                                    handle,
                                                    start_x: coords.x,
                                                    start_y: coords.y,
                                                    orig_x: layer_x,
                                                    orig_y: layer_y,
                                                    orig_w: w,
                                                    orig_h: h,
                                                })));
                                                evt.stop_propagation();
                                            }
                                        },
                                        on_rotate_start: move |evt: MouseEvent| {
                                            if selected_layers.read().len() == 1 {
                                                let coords = evt.page_coordinates();
                                                let (w, h) = match &sigil.read().layers[idx].item {
                                                    Item::Rect(r) => (r.width, r.height),
                                                    Item::Image(i) => (i.width, i.height),
                                                    Item::Text(t) => {
                                                        if let Some(&(tw, th)) = text_dimensions.read().get(&sigil.read().layers[idx].id) {
                                                            (tw, th)
                                                        } else {
                                                            (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
                                                        }
                                                    },
                                                };
                                                let rot_rad = sigil.read().layers[idx].rotation.to_radians();

                                                let dist = h as f64 / 2.0 + 30.0;
                                                let vec_x = dist * (rot_rad.sin() as f64);
                                                let vec_y = -dist * (rot_rad.cos() as f64);
                                                
                                                let center_x = coords.x - vec_x;
                                                let center_y = coords.y - vec_y;
                                                
                                                let start_angle = (coords.y - center_y).atan2(coords.x - center_x);

                                                dragging.set(Some((idx, DragMode::Rotate {
                                                    orig_rotation: layer_rot,
                                                    center_x,
                                                    center_y,
                                                    start_angle,
                                                })));
                                                evt.stop_propagation();
                                            }
                                        }
                                    }
                                }
                            } else {
                                rsx!({})
                            }
                        })
                    }

                    for guide in guides.read().iter() {
                        if guide.is_vertical {
                            div {
                                class: "smart-guide vertical",
                                style: "left: {guide.pos}px; top: {guide.start}px; height: {guide.end - guide.start}px;"
                            }
                        } else {
                            div {
                                class: "smart-guide horizontal",
                                style: "top: {guide.pos}px; left: {guide.start}px; width: {guide.end - guide.start}px;"
                            }
                        }
                    }
                }

                div {
                    class: "json-output",
                    pre {
                        "{serde_json::to_string_pretty(&*sigil.read()).unwrap()}"
                    }
                }
            }
        }
        
        if *show_load_modal.read() {
            div { class: "modal-overlay",
                onclick: move |_| {
                    show_load_modal.set(false);
                },
                div { class: "modal", onclick: move |evt| evt.stop_propagation(),
                    h3 { "Load JSON" }
                    textarea {
                        value: "{load_json_text.read()}",
                        placeholder: "Paste Sigil JSON here...",
                        autofocus: "true",
                        oninput: move |evt| {
                            load_json_text.set(evt.value());
                        }
                    }
                    if let Some(err) = &*load_error.read() {
                        div { class: "error-text", "{err}" }
                    }
                    div { class: "modal-actions",
                        button {
                            class: "primary-btn",
                            onclick: move |_| {
                                match serde_json::from_str::<Sigil>(&load_json_text.read()) {
                                    Ok(new_sigil) => {
                                        sigil.set(new_sigil);
                                        selected_layers.write().clear();
                                        guides.write().clear();
                                        text_dimensions.write().clear();
                                        layer_id_counter.set(sigil.read().layers.len() as i32);
                                        load_error.set(None);
                                        show_load_modal.set(false);
                                    },
                                    Err(e) => {
                                        load_error.set(Some(format!("Invalid JSON: {}", e)));
                                    }
                                }
                            },
                            "Load"
                        }
                        button {
                            onclick: move |_| {
                                show_load_modal.set(false);
                                load_error.set(None);
                            },
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RenderLayer(
    layer: Layer, 
    is_selected: bool, 
    is_locked: bool,
    text_dimensions: Signal<HashMap<String, (f32, f32)>>,
    on_move_start: EventHandler<MouseEvent>,
) -> Element {
    let style = format!(
        "left: {}px; top: {}px; transform: rotate({}deg);",
        layer.x, layer.y, layer.rotation
    );
    let class_name = if is_selected { "layer-render selected" } else { "layer-render" };
    let locked_class = if is_locked { " locked" } else { "" };
    let final_class = format!("{}{}", class_name, locked_class);

    let (w, h) = match &layer.item {
        Item::Rect(r) => (r.width, r.height),
        Item::Image(i) => (i.width, i.height),
        Item::Text(t) => {
            if let Some(&(tw, th)) = text_dimensions.read().get(&layer.id) {
                (tw, th)
            } else {
                (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
            }
        },
    };
    
    let (w_css, h_css) = match &layer.item {
        Item::Text(_) => ("max-content".to_string(), "max-content".to_string()),
        _ => (format!("{}px", w), format!("{}px", h)),
    };
    
    let transform_origin = if let Item::Text(_) = &layer.item { "0 0" } else { "50% 50%" };

    let layer_id = layer.id.clone();
    let item_clone = layer.item.clone();
    
    use_effect(use_reactive(&item_clone, move |item| {
        to_owned![text_dimensions, layer_id];
        spawn(async move {
            if let Item::Text(_) = item {
                let js = format!(
                    "(() => {{
                        const canvas = document.__sigilMeasureCanvas || (document.__sigilMeasureCanvas = document.createElement('canvas'));
                        const ctx = canvas.getContext('2d');
                        ctx.font = '{}px ' + {};
                        
                        const lines = {}.split('\\n');
                        let maxW = 0;
                        let lineH = {};
                        
                        for (const line of lines) {{
                            const m = ctx.measureText(line);
                            maxW = Math.max(maxW, m.width);
                            const h = (m.actualBoundingBoxAscent || 0) + (m.actualBoundingBoxDescent || 0);
                            if (h > lineH) lineH = h;
                        }}
                        
                        const totalH = lineH * Math.max(lines.length, 1);
                        return [maxW, totalH];
                    }})()",
                    match &item {
                        Item::Text(t) => t.font_size,
                        _ => 0.0
                    },
                    match &item {
                        Item::Text(t) => serde_json::to_string(&t.font_family).unwrap_or("\"Sans Serif\"".to_string()),
                        _ => "\"\"".to_string()
                    },
                    match &item {
                        Item::Text(t) => serde_json::to_string(&t.text).unwrap_or("\"\"".to_string()),
                        _ => "\"\"".to_string()
                    },
                    match &item {
                        Item::Text(t) => t.font_size,
                        _ => 0.0
                    }
                );
                
                if let Ok(val) = document::eval(&js).recv().await {
                    if let Ok(dims) = serde_json::from_value::<Vec<f64>>(val) {
                        if dims.len() == 2 {
                            text_dimensions.write().insert(layer_id, (dims[0] as f32, dims[1] as f32));
                        }
                    }
                }
            }
        });
    }));

    rsx! {
        div {
            id: "layer-{layer.id}",
            class: "{final_class}",
            style: "{style} width: {w_css}; height: {h_css}; transform-origin: {transform_origin};",
            onmousedown: move |evt| {
                evt.prevent_default();
                on_move_start.call(evt);
            },
            ondragstart: move |evt| evt.prevent_default(),
            onclick: move |evt| evt.stop_propagation(),
            
            match &layer.item {
                Item::Rect(rect) => rsx! {
                    div {
                        style: "width: 100%; height: 100%; background-color: {rect.color}; border-radius: {rect.border_radius}px;",
                    }
                },
                Item::Text(text) => rsx! {
                    div {
                        style: "font-size: {text.font_size}px; color: {text.color}; font-family: {text.font_family}; white-space: pre; user-select: none;",
                        "{text.text}"
                    }
                },
                Item::Image(img) => rsx! {
                    if img.source.is_empty() {
                        div {
                            class: "image-placeholder",
                            style: "width: 100%; height: 100%; border-radius: {img.border_radius}px;",
                            "No Image Source"
                        }
                    } else {
                        img {
                            style: "width: 100%; height: 100%; border-radius: {img.border_radius}px; object-fit: cover;",
                            src: "{img.source}", 
                            alt: "img",
                            draggable: "false",
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SelectionOverlay(
    layer: Layer,
    text_dimensions: Signal<HashMap<String, (f32, f32)>>,
    on_resize_start: EventHandler<(HandleType, MouseEvent)>,
    on_rotate_start: EventHandler<MouseEvent>,
) -> Element {
    let style = format!(
        "left: {}px; top: {}px; transform: rotate({}deg);",
        layer.x, layer.y, layer.rotation
    );
    
    let (w, h) = match &layer.item {
        Item::Rect(r) => (r.width, r.height),
        Item::Image(i) => (i.width, i.height),
        Item::Text(t) => {
            if let Some(&(tw, th)) = text_dimensions.read().get(&layer.id) {
                (tw, th)
            } else {
                (t.text.len() as f32 * t.font_size * 0.6, t.font_size)
            }
        },
    };

    let (w_css, h_css) = match &layer.item {
        Item::Text(_) => ("max-content".to_string(), "max-content".to_string()),
        _ => (format!("{}px", w), format!("{}px", h)),
    };
    
    let transform_origin = if let Item::Text(_) = &layer.item { "0 0" } else { "50% 50%" };
    
    let show_handles = w > 0.0;

    rsx! {
        div {
            class: "selection-overlay",
            style: "{style} width: {w_css}; height: {h_css}; transform-origin: {transform_origin};",

            if let Item::Text(text) = &layer.item {
                div {
                    style: "font-size: {text.font_size}px; font-family: {text.font_family}; white-space: pre; opacity: 0;",
                    "{text.text}"
                }
            }

            if show_handles {
                div { class: "resize-handle tl", onmousedown: move |evt| on_resize_start.call((HandleType::TopLeft, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle tr", onmousedown: move |evt| on_resize_start.call((HandleType::TopRight, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle bl", onmousedown: move |evt| on_resize_start.call((HandleType::BottomLeft, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle br", onmousedown: move |evt| on_resize_start.call((HandleType::BottomRight, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle t", onmousedown: move |evt| on_resize_start.call((HandleType::Top, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle b", onmousedown: move |evt| on_resize_start.call((HandleType::Bottom, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle l", onmousedown: move |evt| on_resize_start.call((HandleType::Left, evt)), onclick: move |evt| evt.stop_propagation() }
                div { class: "resize-handle r", onmousedown: move |evt| on_resize_start.call((HandleType::Right, evt)), onclick: move |evt| evt.stop_propagation() }
            }

            div { 
                class: "rotate-handle",
                onmousedown: move |evt| on_rotate_start.call(evt),
                onclick: move |evt| evt.stop_propagation(),
            }
            div { class: "rotate-line" }
        }
    }
}

fn item_type_name(item: &Item) -> &'static str {
    match item {
        Item::Rect(_) => "Rectangle",
        Item::Text(_) => "Text",
        Item::Image(_) => "Image",
    }
}
