use nalgebra_glm as glm;
use pollster::FutureExt;
use std::collections::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::preproc::{self, build_shader};

pub(crate) struct WgpuState {
    pub camera_buffer: Buffer,
    pub lights_buffer: Buffer,
    dvo_texture: Texture,
    voxels_texture: Texture,
    colors_texture: Texture,
    vertex_buffer: Buffer,

    uniforms_bind_group: BindGroup,
    dvo_bind_group: BindGroup,

    render_pipeline: RenderPipeline,
    compute_pipeline: ComputePipeline,
}

impl WgpuState {
    pub(crate) fn new(
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
        camera_data: &[u8],
        lights_data: &[u8],
        voxels_data: &[u8],
        dim: u32,
        colors_data: &[u8],
        msaa_level: u32,
    ) -> Self {
        let dvo_depth = dim.ilog2() - 1;
        let render_pipeline =
            create_shader_pipeline(device, surface_config, dvo_depth, msaa_level).unwrap();
        let compute_pipeline = create_compute_pipeline(device, dvo_depth).unwrap();

        let camera_buffer = create_camera_buffer(device, camera_data);
        let lights_buffer = create_lights_buffer(device, lights_data);
        let dvo_texture = create_dvo_texture(device, dim);
        let colors_texture = create_colors_texture(device, queue, dim, colors_data);
        let vertex_buffer = create_vertex_buffer(device);
        let voxels_texture = create_voxels_texture(device, queue, dim, voxels_data);

        let uniforms_bind_group = create_uniforms_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(0),
            &camera_buffer,
            &lights_buffer,
        );
        let dvo_bind_group = create_dvo_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(1),
            &dvo_texture,
            &colors_texture,
        );
        Self {
            camera_buffer,
            lights_buffer,
            dvo_texture,
            voxels_texture,
            colors_texture,
            vertex_buffer,

            uniforms_bind_group,
            dvo_bind_group,

            render_pipeline,
            compute_pipeline,
        }
    }

    pub(crate) fn draw(&self, view: &TextureView, encoder: &mut CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.uniforms_bind_group, &[]);
        render_pass.set_bind_group(1, &self.dvo_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        render_pass.draw(0..6, 0..1);
    }

    fn compute_single_pass(
        &self,
        device: &Device,
        encoder: &mut CommandEncoder,
        input_view: &TextureView,
        output_view: &TextureView,
        dim: u32,
    ) {
        let bind_group = create_compute_bind_group(
            device,
            &self.compute_pipeline.get_bind_group_layout(0),
            &input_view,
            &output_view,
        );

        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("compute pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(dim / 2, dim / 2, dim / 2);
        }
    }

    pub(crate) fn compute(&self, device: &Device, encoder: &mut CommandEncoder, mut dim: u32) {
        let mut depth = 0;

        // first pass
        {
            let input_view = self.voxels_texture.create_view(&TextureViewDescriptor {
                label: Some("input texture view"),
                ..Default::default()
            });

            let output_view = self.dvo_texture.create_view(&TextureViewDescriptor {
                label: Some("output texture view"),
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            });

            println!("compute octree, depth={depth}, dim={dim}");
            self.compute_single_pass(device, encoder, &input_view, &output_view, dim);
            dim /= 2;
            depth += 1;
        }

        while dim > 1 {
            let input_view = self.dvo_texture.create_view(&TextureViewDescriptor {
                label: Some("input texture view"),
                base_mip_level: depth - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let output_view = self.dvo_texture.create_view(&TextureViewDescriptor {
                label: Some("output texture view"),
                base_mip_level: depth,
                mip_level_count: Some(1),
                ..Default::default()
            });

            println!("compute octree, depth={depth}, dim={dim}");
            self.compute_single_pass(device, encoder, &input_view, &output_view, dim);
            dim /= 2;
            depth += 1;
        }
    }

    pub(crate) fn reload_shaders(
        &mut self,
        device: &Device,
        surface_config: &SurfaceConfiguration,
        dvo_depth: u32,
        msaa_level: u32,
    ) {
        if let Some(render_pipeline) =
            create_shader_pipeline(device, surface_config, dvo_depth, msaa_level)
        {
            self.render_pipeline = render_pipeline;
        }
        if let Some(compute_pipeline) = create_compute_pipeline(device, dvo_depth) {
            self.compute_pipeline = compute_pipeline;
        }
    }
}

pub(crate) fn create_colors_texture(
    device: &Device,
    queue: &Queue,
    dim: u32,
    colors_data: &[u8],
) -> Texture {
    let colors_texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some("colors texture"),
            size: Extent3d {
                width: dim,
                height: dim,
                depth_or_array_layers: dim,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        },
        colors_data,
    );

    colors_texture
}

pub(crate) fn create_dvo_texture(device: &Device, dim: u32) -> Texture {
    // 4 bytes per dvo node (1 u32)
    let depth = dim.ilog2();
    let nodes = (8u64.pow(depth) - 1) / 7;
    println!(
        "dvo nodes: {nodes} ({}B = {} MiB), depth={depth}",
        nodes * 4,
        nodes * 4 / 1024 / 1024
    );

    let dvo_texture = device.create_texture(&TextureDescriptor {
        label: Some("dvo texture"),
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
        size: Extent3d {
            width: dim / 2,
            height: dim / 2,
            depth_or_array_layers: dim / 2,
        },
        mip_level_count: depth,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::R8Uint,
        view_formats: &[],
    });

    dvo_texture
}

pub(crate) fn create_vertex_buffer(device: &Device) -> Buffer {
    const BUF_DATA: &[glm::Vec2] = &[
        glm::Vec2::new(-1.0, -1.0),
        glm::Vec2::new(1.0, -1.0),
        glm::Vec2::new(1.0, 1.0),
        glm::Vec2::new(-1.0, -1.0),
        glm::Vec2::new(1.0, 1.0),
        glm::Vec2::new(-1.0, 1.0),
    ];

    let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("vertex buffer"),
        contents: bytemuck::cast_slice(BUF_DATA),
        usage: BufferUsages::VERTEX,
    });

    vertex_buffer
}

pub(crate) fn create_camera_buffer(device: &Device, camera_data: &[u8]) -> Buffer {
    let camera_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("camera buffer"),
        contents: camera_data,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    camera_buffer
}

pub(crate) fn create_lights_buffer(device: &Device, lights_data: &[u8]) -> Buffer {
    let lights_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("lights buffer"),
        contents: lights_data,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    lights_buffer
}

pub(crate) fn create_voxels_texture(
    device: &Device,
    queue: &Queue,
    dim: u32,
    voxels_data: &[u8],
) -> Texture {
    let voxels_texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some("voxels texture"),
            size: Extent3d {
                width: dim,
                height: dim,
                depth_or_array_layers: dim,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::R8Uint,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        },
        voxels_data,
    );

    voxels_texture
}

pub(crate) fn create_uniforms_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    camera_buffer: &Buffer,
    lights_buffer: &Buffer,
) -> BindGroup {
    let uniforms_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("uniforms bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: lights_buffer.as_entire_binding(),
            },
        ],
    });

    uniforms_bind_group
}

pub(crate) fn create_dvo_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    dvo_texture: &Texture,
    colors_texture: &Texture,
) -> BindGroup {
    let dvo_view = dvo_texture.create_view(&TextureViewDescriptor {
        label: Some("dvo texture view"),
        ..Default::default()
    });

    let colors_view = colors_texture.create_view(&TextureViewDescriptor {
        label: Some("colors texture view"),
        ..Default::default()
    });

    let dvo_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("dvo bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&dvo_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&colors_view),
            },
        ],
    });

    dvo_bind_group
}

pub(crate) fn create_compute_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    input_view: &TextureView,
    output_view: &TextureView,
) -> BindGroup {
    let compute_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("compute bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&input_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&output_view),
            },
        ],
    });

    compute_bind_group
}

pub(crate) fn create_shader_pipeline(
    device: &Device,
    surface_config: &SurfaceConfiguration,
    dvo_depth: u32,
    msaa_level: u32,
) -> Option<RenderPipeline> {
    let preproc_ctx = preproc::Context {
        main: "src/shader.wgsl".into(),
        constants: HashMap::from([
            ("DVO_DEPTH".to_owned(), format!("{dvo_depth}u")),
            ("MSAA_LEVEL".to_owned(), format!("{msaa_level}u")),
        ]),
    };
    let shader_source = build_shader(&preproc_ctx).unwrap();

    device.push_error_scope(ErrorFilter::Validation);

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("shader"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });

    let err = device.pop_error_scope().block_on();
    match err {
        Some(err) => {
            eprintln!("{}", err);
            return None;
        }
        None => println!("compiled render shader"),
    }

    let dvo_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("dvo bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                // dvo
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Uint,
                    view_dimension: TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // colors
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadOnly,
                    format: TextureFormat::Rgba8Unorm,
                    view_dimension: TextureViewDimension::D3,
                },
                count: None,
            },
        ],
    });

    let uniforms_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("uniforms bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                // camera
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // lights
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("render pipeline layout"),
        bind_group_layouts: &[&uniforms_bind_group_layout, &dvo_bind_group_layout],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("render pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<glm::Vec2>() as BufferAddress,
                step_mode: VertexStepMode::Vertex,
                attributes: &[VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x2,
                }],
            }],
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: surface_config.format,
                blend: Some(BlendState {
                    color: BlendComponent::REPLACE,
                    alpha: BlendComponent::REPLACE,
                }),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            front_face: FrontFace::Ccw,
            cull_mode: Some(Face::Back),
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
    });

    Some(render_pipeline)
}

fn create_compute_pipeline(device: &Device, dvo_depth: u32) -> Option<ComputePipeline> {
    let preproc_ctx = preproc::Context {
        main: "src/compute.wgsl".into(),
        constants: HashMap::from([("DVO_DEPTH".to_owned(), format!("{dvo_depth}u"))]),
    };
    let shader_source = build_shader(&preproc_ctx).unwrap();

    device.push_error_scope(ErrorFilter::Validation);

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("compute"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });

    let err = device.pop_error_scope().block_on();
    match err {
        Some(err) => {
            eprintln!("{}", err);
            return None;
        }
        None => println!("compiled compute shader"),
    }

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("compute bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                // voxels
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadOnly,
                    format: TextureFormat::R8Uint,
                    view_dimension: TextureViewDimension::D3,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // dvo
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::R8Uint,
                    view_dimension: TextureViewDimension::D3,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("compute pipeline layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("compute pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "cs_main",
    });

    Some(compute_pipeline)
}
