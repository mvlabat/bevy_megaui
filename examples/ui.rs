use bevy::prelude::*;
use bevy_megaui::{
    megaui::{hash, Vector2},
    MegaUiContext, MegaUiPlugin, WindowParams,
};

const BEVY_TEXTURE_ID: u32 = 0;

fn main() {
    App::build()
        .add_resource(ClearColor(Color::rgb(1.0, 1.0, 1.0)))
        .add_plugins(DefaultPlugins)
        .add_plugin(MegaUiPlugin)
        .add_startup_system(load_assets.system())
        .add_system(ui_example.system())
        .run();
}

#[derive(Default)]
struct UiState {
    input1: String,
    input2: String,
    slider1: f32,
    slider2: f32,
    e1_input: String,
    e2_input: String,
    inverted: bool,
}

fn load_assets(_world: &mut World, resources: &mut Resources) {
    let mut megaui_context = resources.get_thread_local_mut::<MegaUiContext>().unwrap();
    let asset_server = resources.get::<AssetServer>().unwrap();

    let texture_handle = asset_server.load("icon.png");
    megaui_context.set_megaui_texture(BEVY_TEXTURE_ID, texture_handle);
}

fn ui_example(_world: &mut World, resources: &mut Resources) {
    resources.get_or_insert_with(UiState::default);

    let mut ui = resources.get_thread_local_mut::<MegaUiContext>().unwrap();
    let mut ui_state = resources.get_mut::<UiState>().unwrap();
    let mut load = false;
    let mut remove = false;
    let mut invert = false;

    ui.draw_window(
        hash!(),
        Vector2::new(360.0, 30.0),
        Vector2::new(300.0, 300.0),
        WindowParams {
            label: "Custom textures".to_owned(),
            ..Default::default()
        },
        |ui| {
            load = ui.button(None, "Load");
            remove = ui.button(Vector2::new(60.0, 1.0), "Remove");
            invert = ui.button(Vector2::new(135.0, 1.0), "Invert");
            ui.separator();
            ui.texture(BEVY_TEXTURE_ID, 256.0, 256.0);
        },
    );

    if invert {
        ui_state.inverted = !ui_state.inverted;
    }
    if load || invert {
        let asset_server = resources.get::<AssetServer>().unwrap();
        let texture_handle = if ui_state.inverted {
            asset_server.load("icon_inverted.png")
        } else {
            asset_server.load("icon.png")
        };
        ui.set_megaui_texture(BEVY_TEXTURE_ID, texture_handle);
    }
    if remove {
        ui.remove_megaui_texture(BEVY_TEXTURE_ID);
    }

    ui.draw_window(
        hash!(),
        Vector2::new(30.0, 30.0),
        Vector2::new(300.0, 300.0),
        WindowParams {
            label: "UI Showcase".to_owned(),
            ..Default::default()
        },
        |ui| {
            ui.tree_node(hash!(), "input", |ui| {
                ui.label(None, "Some random text");
                if ui.button(None, "click me") {
                    println!("hi");
                }

                ui.separator();

                ui.label(None, "Some other random text");
                if ui.button(None, "other button") {
                    println!("hi2");
                }

                ui.separator();

                ui.input_field(hash!(), "<- input text 1", &mut ui_state.input1);
                ui.input_field(hash!(), "<- input text 2", &mut ui_state.input2);
                ui.label(
                    None,
                    &format!(
                        "Text entered: \"{}\" and \"{}\"",
                        ui_state.input1, ui_state.input2
                    ),
                );

                ui.separator();
            });
            ui.tree_node(hash!(), "sliders", |ui| {
                ui.slider(hash!(), "[-10 .. 10]", -10f32..10f32, &mut ui_state.slider1);
                ui.slider(hash!(), "[0 .. 100]", 0f32..100f32, &mut ui_state.slider2);
            });
            ui.tree_node(hash!(), "editbox 1", |ui| {
                ui.label(None, "This is editbox!");
                ui.editbox(
                    hash!(),
                    megaui::Vector2::new(285., 165.),
                    &mut ui_state.e1_input,
                );
            });
            ui.tree_node(hash!(), "editbox 2", |ui| {
                ui.label(None, "This is editbox!");
                ui.editbox(
                    hash!(),
                    megaui::Vector2::new(285., 165.),
                    &mut ui_state.e2_input,
                );
            });
        },
    );
}
