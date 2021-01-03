use crate::{
    MegaUiContext, MegaUiSettings, WindowSize, MEGAUI_PIPELINE_HANDLE,
    MEGAUI_TEXTURE_RESOURCE_BINDING_NAME, MEGAUI_TRANSFORM_RESOURCE_BINDING_NAME,
};
use bevy::{
    app::{EventReader, Events},
    asset::{AssetEvent, Assets, Handle},
    core::{AsBytes, Time},
    ecs::{Resources, World},
    log,
    render::{
        pass::{
            ClearColor, LoadOp, Operations, PassDescriptor,
            RenderPassDepthStencilAttachmentDescriptor, TextureAttachment,
        },
        pipeline::{
            BindGroupDescriptor, IndexFormat, InputStepMode, PipelineCompiler, PipelineDescriptor,
            PipelineLayout, PipelineSpecialization, VertexAttributeDescriptor,
            VertexBufferDescriptor, VertexFormat,
        },
        render_graph::{base::Msaa, Node, ResourceSlotInfo, ResourceSlots},
        renderer::{
            BindGroup, BufferId, BufferInfo, BufferUsage, RenderContext, RenderResourceBinding,
            RenderResourceBindings, RenderResourceType, SamplerId, TextureId,
        },
        shader::Shader,
        texture::{Texture, TextureDescriptor},
    },
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

pub struct MegaUiNode {
    pass_descriptor: PassDescriptor,
    pipeline_descriptor: Option<Handle<PipelineDescriptor>>,
    inputs: Vec<ResourceSlotInfo>,
    color_attachment_input_indices: Vec<Option<usize>>,
    color_resolve_target_indices: Vec<Option<usize>>,
    depth_stencil_attachment_input_index: Option<usize>,
    default_clear_color_inputs: Vec<usize>,

    transform_bind_group_descriptor: Option<BindGroupDescriptor>,
    transform_bind_group: Option<BindGroup>,

    font_texture: Handle<Texture>,
    texture_bind_group_descriptor: Option<BindGroupDescriptor>,
    texture_resources: HashMap<Handle<Texture>, TextureResource>,
    event_reader: EventReader<AssetEvent<Texture>>,

    vertex_buffer: Option<BufferId>,
    index_buffer: Option<BufferId>,
}

#[derive(Debug)]
pub struct TextureResource {
    descriptor: TextureDescriptor,
    texture: TextureId,
    sampler: SamplerId,
    bind_group: BindGroup,
}

impl MegaUiNode {
    pub fn new(msaa: &Msaa, font_texture: Handle<Texture>) -> Self {
        let color_attachments = vec![msaa.color_attachment_descriptor(
            TextureAttachment::Input("color_attachment".to_string()),
            TextureAttachment::Input("color_resolve_target".to_string()),
            Operations {
                load: LoadOp::Load,
                store: true,
            },
        )];
        let depth_stencil_attachment = RenderPassDepthStencilAttachmentDescriptor {
            attachment: TextureAttachment::Input("depth".to_string()),
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        };

        let mut inputs = Vec::new();
        let mut color_attachment_input_indices = Vec::new();
        let mut color_resolve_target_indices = Vec::new();

        for color_attachment in color_attachments.iter() {
            if let TextureAttachment::Input(ref name) = color_attachment.attachment {
                color_attachment_input_indices.push(Some(inputs.len()));
                inputs.push(ResourceSlotInfo::new(
                    name.to_string(),
                    RenderResourceType::Texture,
                ));
            } else {
                color_attachment_input_indices.push(None);
            }

            if let Some(TextureAttachment::Input(ref name)) = color_attachment.resolve_target {
                color_resolve_target_indices.push(Some(inputs.len()));
                inputs.push(ResourceSlotInfo::new(
                    name.to_string(),
                    RenderResourceType::Texture,
                ));
            } else {
                color_resolve_target_indices.push(None);
            }
        }

        let mut depth_stencil_attachment_input_index = None;
        if let TextureAttachment::Input(ref name) = depth_stencil_attachment.attachment {
            depth_stencil_attachment_input_index = Some(inputs.len());
            inputs.push(ResourceSlotInfo::new(
                name.to_string(),
                RenderResourceType::Texture,
            ));
        }

        Self {
            pass_descriptor: PassDescriptor {
                color_attachments,
                depth_stencil_attachment: Some(depth_stencil_attachment),
                sample_count: msaa.samples,
            },
            pipeline_descriptor: None,
            default_clear_color_inputs: Vec::new(),
            inputs,
            depth_stencil_attachment_input_index,
            color_attachment_input_indices,
            transform_bind_group_descriptor: None,
            transform_bind_group: None,
            font_texture,
            texture_bind_group_descriptor: None,
            texture_resources: Default::default(),
            event_reader: Default::default(),
            vertex_buffer: None,
            index_buffer: None,
            color_resolve_target_indices,
        }
    }
}

struct DrawCommand {
    vertices_count: usize,
    texture_handle: Option<Handle<Texture>>,
    clipping_zone: Option<megaui::Rect>,
}

impl Node for MegaUiNode {
    fn input(&self) -> &[ResourceSlotInfo] {
        &self.inputs
    }

    fn update(
        &mut self,
        _world: &World,
        resources: &Resources,
        render_context: &mut dyn RenderContext,
        input: &ResourceSlots,
        _output: &mut ResourceSlots,
    ) {
        self.process_attachments(input, resources);
        self.init_pipeline(render_context, resources);

        let window_size = resources.get::<WindowSize>().unwrap();
        let megaui_settings = resources.get::<MegaUiSettings>().unwrap();

        let render_resource_bindings = resources.get::<RenderResourceBindings>().unwrap();

        self.init_transform_bind_group(render_context, &render_resource_bindings);

        let texture_assets = resources.get_mut::<Assets<Texture>>().unwrap();
        let asset_events = resources.get::<Events<AssetEvent<Texture>>>().unwrap();

        let mut megaui_context = resources.get_thread_local_mut::<MegaUiContext>().unwrap();

        self.process_asset_events(
            render_context,
            &mut megaui_context,
            &asset_events,
            &texture_assets,
        );
        self.remove_unused_textures(render_context, &megaui_context);
        self.init_textures(render_context, &megaui_context, &texture_assets);

        megaui_context.render_draw_lists();
        let mut ui_draw_lists = Vec::new();

        std::mem::swap(&mut ui_draw_lists, &mut megaui_context.ui_draw_lists);

        let mut vertex_buffer = Vec::<u8>::new();
        let mut index_buffer = Vec::new();
        let mut draw_commands = Vec::new();
        let mut index_offset = 0;

        for draw_list in &ui_draw_lists {
            let texture_handle = if let Some(texture) = draw_list.texture {
                megaui_context.megaui_textures.get(&texture).cloned()
            } else {
                Some(megaui_context.font_texture.clone())
            };

            for vertex in &draw_list.vertices {
                vertex_buffer.extend_from_slice(vertex.pos.as_bytes());
                vertex_buffer.extend_from_slice(vertex.uv.as_bytes());
                vertex_buffer.extend_from_slice(vertex.color.as_bytes());
            }
            let indices_with_offset = draw_list
                .indices
                .iter()
                .map(|i| i + index_offset)
                .collect::<Vec<_>>();
            index_buffer.extend_from_slice(indices_with_offset.as_slice().as_bytes());
            index_offset += draw_list.vertices.len() as u16;

            draw_commands.push(DrawCommand {
                vertices_count: draw_list.indices.len(),
                texture_handle,
                clipping_zone: draw_list.clipping_zone,
            });
        }

        self.update_buffers(render_context, &vertex_buffer, &index_buffer);

        render_context.begin_pass(
            &self.pass_descriptor,
            &render_resource_bindings,
            &mut |render_pass| {
                render_pass.set_pipeline(self.pipeline_descriptor.as_ref().unwrap());
                render_pass.set_vertex_buffer(0, self.vertex_buffer.unwrap(), 0);
                render_pass.set_index_buffer(self.index_buffer.unwrap(), 0);
                render_pass.set_bind_group(
                    0,
                    self.transform_bind_group_descriptor.as_ref().unwrap().id,
                    self.transform_bind_group.as_ref().unwrap().id,
                    None,
                );

                // This is a pretty weird kludge, but we need to bind all our groups at least once,
                // so they don't get garbage collected by `remove_stale_bind_groups`.
                for texture_resource in self.texture_resources.values() {
                    render_pass.set_bind_group(
                        1,
                        self.texture_bind_group_descriptor.as_ref().unwrap().id,
                        texture_resource.bind_group.id,
                        None,
                    );
                }

                let mut vertex_offset: u32 = 0;
                for draw_command in &draw_commands {
                    let texture_resource = match draw_command
                        .texture_handle
                        .as_ref()
                        .and_then(|texture_handle| self.texture_resources.get(texture_handle))
                    {
                        Some(texture_resource) => texture_resource,
                        None => {
                            vertex_offset += draw_command.vertices_count as u32;
                            continue;
                        }
                    };

                    render_pass.set_bind_group(
                        1,
                        self.texture_bind_group_descriptor.as_ref().unwrap().id,
                        texture_resource.bind_group.id,
                        None,
                    );

                    let scale_factor =
                        window_size.scale_factor * megaui_settings.scale_factor as f32;
                    if let Some(clipping_zone) = draw_command.clipping_zone {
                        render_pass.set_scissor_rect(
                            (clipping_zone.x * scale_factor) as u32,
                            (clipping_zone.y * scale_factor) as u32,
                            (clipping_zone.w * scale_factor) as u32,
                            (clipping_zone.h * scale_factor) as u32,
                        );
                    } else {
                        render_pass.set_scissor_rect(
                            0,
                            0,
                            window_size.physical_width as u32,
                            window_size.physical_height as u32,
                        );
                    }
                    render_pass.draw_indexed(
                        vertex_offset..(vertex_offset + draw_command.vertices_count as u32),
                        0,
                        0..1,
                    );
                    vertex_offset += draw_command.vertices_count as u32;
                }
            },
        );

        std::mem::swap(&mut ui_draw_lists, &mut megaui_context.ui_draw_lists);
        megaui_context
            .ui
            .new_frame(resources.get::<Time>().unwrap().delta_seconds());
    }
}

impl MegaUiNode {
    fn process_attachments(&mut self, input: &ResourceSlots, resources: &Resources) {
        if let Some(input_index) = self.depth_stencil_attachment_input_index {
            self.pass_descriptor
                .depth_stencil_attachment
                .as_mut()
                .unwrap()
                .attachment =
                TextureAttachment::Id(input.get(input_index).unwrap().get_texture().unwrap());
        }

        for (i, color_attachment) in self
            .pass_descriptor
            .color_attachments
            .iter_mut()
            .enumerate()
        {
            if self.default_clear_color_inputs.contains(&i) {
                if let Some(default_clear_color) = resources.get::<ClearColor>() {
                    color_attachment.ops.load = LoadOp::Clear(default_clear_color.0);
                }
            }
            if let Some(input_index) = self.color_attachment_input_indices[i] {
                color_attachment.attachment =
                    TextureAttachment::Id(input.get(input_index).unwrap().get_texture().unwrap());
            }
            if let Some(input_index) = self.color_resolve_target_indices[i] {
                color_attachment.resolve_target = Some(TextureAttachment::Id(
                    input.get(input_index).unwrap().get_texture().unwrap(),
                ));
            }
        }
    }

    fn init_pipeline(&mut self, render_context: &mut dyn RenderContext, resources: &Resources) {
        if self.pipeline_descriptor.is_some() {
            return;
        }

        let mut pipelines = resources.get_mut::<Assets<PipelineDescriptor>>().unwrap();
        let mut shaders = resources.get_mut::<Assets<Shader>>().unwrap();
        let msaa = resources.get::<Msaa>().unwrap();

        let pipeline_descriptor_handle = {
            let render_resource_context = render_context.resources();
            let mut pipeline_compiler = resources.get_mut::<PipelineCompiler>().unwrap();

            let attributes = vec![
                VertexAttributeDescriptor {
                    name: Cow::from("Vertex_Position"),
                    offset: 0,
                    format: VertexFormat::Float3,
                    shader_location: 0,
                },
                VertexAttributeDescriptor {
                    name: Cow::from("Vertex_Uv"),
                    offset: VertexFormat::Float3.get_size(),
                    format: VertexFormat::Float2,
                    shader_location: 1,
                },
                VertexAttributeDescriptor {
                    name: Cow::from("Vertex_Color"),
                    offset: VertexFormat::Float3.get_size() + VertexFormat::Float2.get_size(),
                    format: VertexFormat::Float4,
                    shader_location: 2,
                },
            ];
            pipeline_compiler.compile_pipeline(
                render_resource_context,
                &mut pipelines,
                &mut shaders,
                &MEGAUI_PIPELINE_HANDLE.typed(),
                &PipelineSpecialization {
                    vertex_buffer_descriptor: VertexBufferDescriptor {
                        name: Cow::from("MegaUiVertex"),
                        stride: attributes
                            .iter()
                            .fold(0, |acc, attribute| acc + attribute.format.get_size()),
                        step_mode: InputStepMode::Vertex,
                        attributes,
                    },
                    index_format: IndexFormat::Uint16,
                    sample_count: msaa.samples,
                    ..PipelineSpecialization::default()
                },
            )
        };

        let pipeline_descriptor = pipelines.get(pipeline_descriptor_handle.clone()).unwrap();
        let layout = pipeline_descriptor.layout.as_ref().unwrap();
        let transform_bind_group =
            find_bind_group_by_binding_name(layout, MEGAUI_TRANSFORM_RESOURCE_BINDING_NAME)
                .unwrap();
        let texture_bind_group =
            find_bind_group_by_binding_name(layout, MEGAUI_TEXTURE_RESOURCE_BINDING_NAME).unwrap();

        self.pipeline_descriptor = Some(pipeline_descriptor_handle);
        self.transform_bind_group_descriptor = Some(transform_bind_group);
        self.texture_bind_group_descriptor = Some(texture_bind_group);
    }

    fn init_transform_bind_group(
        &mut self,
        render_context: &mut dyn RenderContext,
        render_resource_bindings: &RenderResourceBindings,
    ) {
        if self.transform_bind_group.is_none() {
            let transform_bindings = render_resource_bindings
                .get(MEGAUI_TRANSFORM_RESOURCE_BINDING_NAME)
                .unwrap()
                .clone();
            let transform_bind_group = BindGroup::build()
                .add_binding(0, transform_bindings)
                .finish();
            self.transform_bind_group = Some(transform_bind_group);
        }
        render_context.resources().create_bind_group(
            self.transform_bind_group_descriptor.as_ref().unwrap().id,
            self.transform_bind_group.as_ref().unwrap(),
        );
    }

    fn process_asset_events(
        &mut self,
        render_context: &mut dyn RenderContext,
        megaui_context: &mut MegaUiContext,
        asset_events: &Events<AssetEvent<Texture>>,
        texture_assets: &Assets<Texture>,
    ) {
        let mut changed_assets: HashMap<Handle<Texture>, &Texture> = HashMap::new();
        for event in self.event_reader.iter(asset_events) {
            let handle = match event {
                AssetEvent::Created { ref handle }
                | AssetEvent::Modified { ref handle }
                | AssetEvent::Removed { ref handle } => handle,
            };
            if !self.texture_resources.contains_key(handle) {
                continue;
            }
            log::debug!("{:?}", event);

            match event {
                AssetEvent::Created { .. } => {
                    // Don't have to do anything really, since we track uninitialized textures
                    // via `MegaUiContext::set_megaui_texture` and `Self::init_textures`.
                }
                AssetEvent::Modified { ref handle } => {
                    if let Some(asset) = texture_assets.get(handle) {
                        changed_assets.insert(handle.clone(), asset);
                    }
                }
                AssetEvent::Removed { ref handle } => {
                    megaui_context.remove_texture(handle);
                    self.remove_texture(render_context, handle);
                    // If an asset was modified and removed in the same update, ignore the modification.
                    changed_assets.remove(&handle);
                }
            }
        }
        for (texture_handle, texture) in changed_assets {
            self.update_texture(render_context, texture, texture_handle);
        }
    }

    fn remove_unused_textures(
        &mut self,
        render_context: &mut dyn RenderContext,
        megaui_context: &MegaUiContext,
    ) {
        let texture_handles = megaui_context
            .megaui_textures
            .values()
            .collect::<HashSet<_>>();
        let mut textures_to_remove = Vec::new();

        for texture_handle in self.texture_resources.keys() {
            if !texture_handles.contains(texture_handle)
                && megaui_context.font_texture != *texture_handle
            {
                textures_to_remove.push(texture_handle.clone_weak());
            }
        }
        for texture_to_remove in textures_to_remove {
            self.remove_texture(render_context, &texture_to_remove);
        }
    }

    fn init_textures(
        &mut self,
        render_context: &mut dyn RenderContext,
        megaui_context: &MegaUiContext,
        texture_assets: &Assets<Texture>,
    ) {
        self.create_texture(render_context, texture_assets, self.font_texture.clone());

        for texture in megaui_context.megaui_textures.values() {
            self.create_texture(render_context, texture_assets, texture.clone_weak());
        }
    }

    fn update_texture(
        &mut self,
        render_context: &mut dyn RenderContext,
        texture_asset: &Texture,
        texture_handle: Handle<Texture>,
    ) {
        let texture_resource = match self.texture_resources.get(&texture_handle) {
            Some(texture_resource) => texture_resource,
            None => return,
        };
        log::debug!("Updating a texture: ${:?}", texture_handle);

        let texture_descriptor: TextureDescriptor = texture_asset.into();

        if texture_descriptor != texture_resource.descriptor {
            log::debug!(
                "Removing an updated texture for it to be re-created later: {:?}",
                texture_handle
            );
            // If a texture descriptor is updated, we'll re-create the texture in `init_textures`.
            self.remove_texture(render_context, &texture_handle);
            return;
        }
        Self::copy_texture(render_context, &texture_resource, texture_asset);
    }

    fn create_texture(
        &mut self,
        render_context: &mut dyn RenderContext,
        texture_assets: &Assets<Texture>,
        texture_handle: Handle<Texture>,
    ) {
        if let Some(texture_resource) = self.texture_resources.get(&texture_handle) {
            // bevy_webgl2 seems to clean bind groups each frame.
            render_context.resources().create_bind_group(
                self.texture_bind_group_descriptor.as_ref().unwrap().id,
                &texture_resource.bind_group,
            );
            return;
        }

        // If a texture is still loading, we skip it.
        let texture_asset = match texture_assets.get(texture_handle.clone()) {
            Some(texture_asset) => texture_asset,
            None => return,
        };

        log::debug!("Creating a texture: ${:?}", texture_handle);

        let render_resource_context = render_context.resources();

        let texture_descriptor: TextureDescriptor = texture_asset.into();
        let texture = render_resource_context.create_texture(texture_descriptor);
        let sampler = render_resource_context.create_sampler(&texture_asset.sampler);

        let texture_bind_group = BindGroup::build()
            .add_binding(0, RenderResourceBinding::Texture(texture))
            .add_binding(1, RenderResourceBinding::Sampler(sampler))
            .finish();

        render_resource_context.create_bind_group(
            self.texture_bind_group_descriptor.as_ref().unwrap().id,
            &texture_bind_group,
        );

        let texture_resource = TextureResource {
            descriptor: texture_descriptor,
            texture,
            sampler,
            bind_group: texture_bind_group,
        };
        Self::copy_texture(render_context, &texture_resource, texture_asset);
        log::debug!("Texture created: {:?}", texture_resource);
        self.texture_resources
            .insert(texture_handle, texture_resource);
    }

    fn remove_texture(
        &mut self,
        render_context: &mut dyn RenderContext,
        texture_handle: &Handle<Texture>,
    ) {
        let texture_resource = match self.texture_resources.remove(texture_handle) {
            Some(texture_resource) => texture_resource,
            None => return,
        };
        log::debug!("Removing a texture: ${:?}", texture_handle);

        let render_resource_context = render_context.resources();
        render_resource_context.remove_texture(texture_resource.texture);
        render_resource_context.remove_sampler(texture_resource.sampler);
    }

    fn copy_texture(
        render_context: &mut dyn RenderContext,
        texture_resource: &TextureResource,
        texture: &Texture,
    ) {
        let aligned_width = render_context
            .resources()
            .get_aligned_texture_size(texture.size.width as usize);
        let format_size = texture.format.pixel_size();

        let texture_buffer = render_context.resources().create_buffer_with_data(
            BufferInfo {
                buffer_usage: BufferUsage::COPY_SRC,
                ..Default::default()
            },
            &texture.data,
        );

        render_context.copy_buffer_to_texture(
            texture_buffer,
            0,
            (format_size * aligned_width) as u32,
            texture_resource.texture,
            [0, 0, 0],
            0,
            texture_resource.descriptor.size,
        );
        render_context.resources().remove_buffer(texture_buffer);
    }

    fn update_buffers(
        &mut self,
        render_context: &mut dyn RenderContext,
        vertex_buffer: &[u8],
        index_buffer: &[u8],
    ) {
        if let Some(vertex_buffer) = self.vertex_buffer.take() {
            render_context.resources().remove_buffer(vertex_buffer);
        }
        if let Some(index_buffer) = self.index_buffer.take() {
            render_context.resources().remove_buffer(index_buffer);
        }
        self.vertex_buffer = Some(render_context.resources().create_buffer_with_data(
            BufferInfo {
                buffer_usage: BufferUsage::VERTEX,
                ..Default::default()
            },
            vertex_buffer,
        ));
        self.index_buffer = Some(render_context.resources().create_buffer_with_data(
            BufferInfo {
                buffer_usage: BufferUsage::INDEX,
                ..Default::default()
            },
            index_buffer,
        ));
    }
}

fn find_bind_group_by_binding_name(
    pipeline_layout: &PipelineLayout,
    binding_name: &str,
) -> Option<BindGroupDescriptor> {
    pipeline_layout
        .bind_groups
        .iter()
        .find(|bind_group| {
            bind_group
                .bindings
                .iter()
                .any(|binding| binding.name == binding_name)
        })
        .cloned()
}
