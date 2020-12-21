# `bevy_megaui`

This crate provides a [megaui](https://crates.io/crates/megaui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.

`bevy_megaui` depends solely on `megaui` and `bevy` with only `render` feature required.

![bevy_megaui](bevy_megaui.png)

## Usage

Here's a minimal usage example:

```rust
use bevy::prelude::*;
use bevy_megaui::{
    megaui::{hash, Vector2},
    MegaUiContext, MegaUiPlugin,
};

fn main() {
    App::build()
        .add_plugins(DefaultPlugins)
        .add_plugin(MegaUiPlugin)
        .add_system(ui_example.system())
        .run();
}

fn ui_example(_world: &mut World, resources: &mut Resources) {
    let mut ui = resources.get_thread_local_mut::<MegaUiContext>().unwrap();

    ui.draw_window(
        hash!(),
        Vector2::new(5.0, 5.0),
        Vector2::new(100.0, 50.0),
        None,
        |ui| {
            ui.label(None, "Hello world!");
        },
    );
}
```

For a more advanced example, see [examples/ui.rs](examples/ui.rs).

```bash
cargo run --example ui --features="bevy/x11 bevy/png bevy/bevy_wgpu"
```
