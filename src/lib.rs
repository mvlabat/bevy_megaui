#![deny(missing_docs)]

//! This crate provides a [megaui](https://crates.io/crates/megaui) integration for the [Bevy](https://github.com/bevyengine/bevy) game engine.
//!
//! `bevy_megaui` depends solely on `megaui` and `bevy` with only `render` feature required.
//!
//! ## Usage
//!
//! Here's a minimal usage example:
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_megaui::{
//!     megaui::{hash, Vector2},
//!     MegaUiContext, MegaUiPlugin,
//! };
//!
//! fn main() {
//!     App::build()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugin(MegaUiPlugin)
//!         .add_system(ui_example.system())
//!         .run();
//! }
//!
//! fn ui_example(_world: &mut World, resources: &mut Resources) {
//!     let mut ui = resources.get_thread_local_mut::<MegaUiContext>().unwrap();
//!
//!     ui.draw_window(
//!         hash!(),
//!         Vector2::new(5.0, 5.0),
//!         Vector2::new(100.0, 50.0),
//!         None,
//!         |ui| {
//!             ui.label(None, "Hello world!");
//!         },
//!     );
//! }
//! ```
//!
//! For a more advanced example, see [examples/ui.rs](examples/ui.rs).
//!
//! ```bash
//! cargo run --example ui --features="bevy/x11 bevy/png bevy/bevy_wgpu"
//! ```

pub use megaui;

mod input;
mod megaui_node;
mod transform_node;

use crate::{input::process_input, megaui_node::MegaUiNode, transform_node::MegaUiTransformNode};
use bevy::{
    app::{stage, AppBuilder, EventReader, Plugin},
    asset::{Assets, Handle, HandleUntyped},
    ecs::IntoSystem,
    log,
    reflect::TypeUuid,
    render::{
        pipeline::{
            BlendDescriptor, BlendFactor, BlendOperation, ColorStateDescriptor, ColorWrite,
            CompareFunction, CullMode, DepthStencilStateDescriptor, FrontFace, IndexFormat,
            PipelineDescriptor, RasterizationStateDescriptor, StencilStateDescriptor,
            StencilStateFaceDescriptor,
        },
        render_graph::{base, base::Msaa, RenderGraph, WindowSwapChainNode, WindowTextureNode},
        shader::{Shader, ShaderStage, ShaderStages},
        texture::{Extent3d, Texture, TextureDimension, TextureFormat},
    },
    window::{CursorMoved, ReceivedCharacter, WindowResized},
};
use megaui::Vector2;
use std::collections::HashMap;

/// A handle pointing to the megaui [PipelineDescriptor].
pub const MEGAUI_PIPELINE_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(PipelineDescriptor::TYPE_UUID, 9404026720151354217);
/// Name of the transform uniform.
pub const MEGAUI_TRANSFORM_RESOURCE_BINDING_NAME: &str = "MegaUiTransform";
/// Name of the texture uniform.
pub const MEGAUI_TEXTURE_RESOURCE_BINDING_NAME: &str = "MegaUiTexture_texture";

/// Adds all megaui resources and render graph nodes.
pub struct MegaUiPlugin;

/// A resource that is used to store `bevy_megaui` context.
/// Since [megaui::Ui] doesn't implement [Send] + [Sync], it's accessible only from
/// thread-local systems.
pub struct MegaUiContext {
    /// Megaui context.
    pub ui: megaui::Ui,
    ui_draw_lists: Vec<megaui::DrawList>,
    font_texture: Handle<Texture>,
    megaui_textures: HashMap<u32, Handle<Texture>>,

    mouse_position: (f32, f32),
    cursor: EventReader<CursorMoved>,
    received_character: EventReader<ReceivedCharacter>,
    resize: EventReader<WindowResized>,
}

impl MegaUiContext {
    fn new(ui: megaui::Ui, font_texture: Handle<Texture>) -> Self {
        Self {
            ui,
            ui_draw_lists: Vec::new(),
            font_texture,
            megaui_textures: Default::default(),
            mouse_position: (0.0, 0.0),
            cursor: Default::default(),
            received_character: Default::default(),
            resize: Default::default(),
        }
    }

    /// A helper function to draw a megaui window.
    /// You may as well use [megaui::widgets::Window::new] if you prefer a builder pattern.
    pub fn draw_window(
        &mut self,
        id: megaui::Id,
        position: Vector2,
        size: Vector2,
        params: impl Into<Option<WindowParams>>,
        f: impl FnOnce(&mut megaui::Ui),
    ) {
        let params = params.into();

        megaui::widgets::Window::new(id, position, size)
            .label(params.as_ref().map_or("", |params| &params.label))
            .titlebar(params.as_ref().map_or(true, |params| params.titlebar))
            .movable(params.as_ref().map_or(true, |params| params.movable))
            .close_button(params.as_ref().map_or(false, |params| params.close_button))
            .ui(&mut self.ui, f);
    }

    /// Can accept either a strong or a weak handle.
    ///
    /// You may want to pass a weak handle if you control removing texture assets in your
    /// application manually and you don't want to bother with cleaning up textures in megaui.
    ///
    /// You'll want to pass a strong handle if a texture is used only in megaui and there's no
    /// handle copies stored anywhere else.
    pub fn set_megaui_texture(&mut self, id: u32, texture: Handle<Texture>) {
        log::debug!("Set megaui texture: {:?}", texture);
        self.megaui_textures.insert(id, texture);
    }

    /// Removes a texture handle associated with the id.
    pub fn remove_megaui_texture(&mut self, id: u32) {
        let texture_handle = self.megaui_textures.remove(&id);
        log::debug!("Remove megaui texture: {:?}", texture_handle);
    }

    // Is called when we get an event that a texture asset is removed.
    fn remove_texture(&mut self, texture_handle: &Handle<Texture>) {
        log::debug!("Removing megaui handles: {:?}", texture_handle);
        self.megaui_textures = self
            .megaui_textures
            .iter()
            .map(|(id, texture)| (*id, texture.clone()))
            .filter(|(_, texture)| texture != texture_handle)
            .collect();
    }
}

/// Params that are used for defining a window with [MegaUiContext::draw_window].
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WindowParams {
    /// Window label (empty by default).
    pub label: String,
    /// Defines whether a window is movable (`true` by default).
    pub movable: bool,
    /// Defines whether a window is closable (`false` by default).
    pub close_button: bool,
    /// Defines whether a window has a titlebar (`true` by default).
    pub titlebar: bool,
}

impl Default for WindowParams {
    fn default() -> WindowParams {
        WindowParams {
            label: "".to_string(),
            movable: true,
            close_button: false,
            titlebar: true,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct WindowSize {
    width: f32,
    height: f32,
    scale_factor: f32,
}

impl WindowSize {
    fn new(width: f32, height: f32, scale_factor: f32) -> Self {
        Self {
            width,
            height,
            scale_factor,
        }
    }
}

impl MegaUiContext {
    fn render_draw_lists(&mut self) {
        self.ui_draw_lists.clear();
        self.ui.render(&mut self.ui_draw_lists);
    }
}

/// The names of `bevy_megaui` nodes.
pub mod node {
    /// The main megaui pass.
    pub const MEGAUI_PASS: &str = "megaui_pass";
    /// Keeps the transform uniform up to date.
    pub const MEGAUI_TRANSFORM: &str = "megaui_transform";
}

impl Plugin for MegaUiPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system_to_stage(stage::PRE_UPDATE, process_input.system());

        let resources = app.resources_mut();

        let ui = megaui::Ui::new();
        let font_texture = {
            let mut assets = resources.get_mut::<Assets<Texture>>().unwrap();
            assets.add(Texture::new(
                Extent3d::new(ui.font_atlas.texture.width, ui.font_atlas.texture.height, 1),
                TextureDimension::D2,
                ui.font_atlas.texture.data.clone(),
                TextureFormat::Rgba8Unorm,
            ))
        };
        resources.insert(WindowSize::new(0.0, 0.0, 0.0));
        resources.insert_thread_local(MegaUiContext::new(ui, font_texture.clone()));

        let mut pipelines = resources.get_mut::<Assets<PipelineDescriptor>>().unwrap();
        let mut shaders = resources.get_mut::<Assets<Shader>>().unwrap();
        let msaa = resources.get::<Msaa>().unwrap();

        pipelines.set_untracked(
            MEGAUI_PIPELINE_HANDLE,
            build_megaui_pipeline(&mut shaders, msaa.samples),
        );
        let mut render_graph = resources.get_mut::<RenderGraph>().unwrap();

        render_graph.add_node(node::MEGAUI_PASS, MegaUiNode::new(&msaa, font_texture));
        render_graph
            .add_node_edge(base::node::MAIN_PASS, node::MEGAUI_PASS)
            .unwrap();

        render_graph
            .add_slot_edge(
                base::node::PRIMARY_SWAP_CHAIN,
                WindowSwapChainNode::OUT_TEXTURE,
                node::MEGAUI_PASS,
                if msaa.samples > 1 {
                    "color_resolve_target"
                } else {
                    "color_attachment"
                },
            )
            .unwrap();

        render_graph
            .add_slot_edge(
                base::node::MAIN_DEPTH_TEXTURE,
                WindowTextureNode::OUT_TEXTURE,
                node::MEGAUI_PASS,
                "depth",
            )
            .unwrap();

        if msaa.samples > 1 {
            render_graph
                .add_slot_edge(
                    base::node::MAIN_SAMPLED_COLOR_ATTACHMENT,
                    WindowSwapChainNode::OUT_TEXTURE,
                    node::MEGAUI_PASS,
                    "color_attachment",
                )
                .unwrap();
        }

        // Transform.
        render_graph.add_system_node(node::MEGAUI_TRANSFORM, MegaUiTransformNode::new());
        render_graph
            .add_node_edge(node::MEGAUI_TRANSFORM, node::MEGAUI_PASS)
            .unwrap();
    }
}

fn build_megaui_pipeline(shaders: &mut Assets<Shader>, sample_count: u32) -> PipelineDescriptor {
    PipelineDescriptor {
        rasterization_state: Some(RasterizationStateDescriptor {
            front_face: FrontFace::Cw,
            cull_mode: CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
            clamp_depth: false,
        }),
        depth_stencil_state: Some(DepthStencilStateDescriptor {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: CompareFunction::LessEqual,
            stencil: StencilStateDescriptor {
                front: StencilStateFaceDescriptor::IGNORE,
                back: StencilStateFaceDescriptor::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
        }),
        color_states: vec![ColorStateDescriptor {
            format: TextureFormat::default(),
            color_blend: BlendDescriptor {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha_blend: BlendDescriptor {
                src_factor: BlendFactor::OneMinusDstAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            write_mask: ColorWrite::ALL,
        }],
        index_format: IndexFormat::Uint16,
        sample_count,
        ..PipelineDescriptor::new(ShaderStages {
            vertex: shaders.add(Shader::from_glsl(
                ShaderStage::Vertex,
                if cfg!(target_arch = "wasm32") {
                    include_str!("megaui.es.vert")
                } else {
                    include_str!("megaui.vert")
                },
            )),
            fragment: Some(shaders.add(Shader::from_glsl(
                ShaderStage::Fragment,
                if cfg!(target_arch = "wasm32") {
                    include_str!("megaui.es.frag")
                } else {
                    include_str!("megaui.frag")
                },
            ))),
        })
    }
}
