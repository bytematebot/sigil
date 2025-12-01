use dioxus::prelude::*;
use sigil_editor::SigilEditor;

fn main() {
    dioxus::launch(App);
}

fn App() -> Element {
    rsx! {
        style {
            "{{
                body, html {{
                    margin: 0;
                    padding: 0;
                    height: 100%;
                    width: 100%;
                    overflow: hidden;
                }}
            }}"
        }
        SigilEditor {}
    }
}
