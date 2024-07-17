use nalgebra_glm as glm;
use std::collections::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::preproc::{self, build_shader};

pub(crate) struct WgpuState {
    pub camera_buffer: Buffer,
    dvo_buffer: Buffer,
    palette_buffer: Buffer,
    vertex_buffer: Buffer,

    camera_bind_group: BindGroup,
    dvo_bind_group: BindGroup,
    voxels_bind_group: BindGroup,

    render_pipeline: RenderPipeline,
    compute_pipeline: ComputePipeline,
}

impl WgpuState {
    pub(crate) fn new(
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
        camera_data: &[u8],
        voxels_data: &[u8],
        dim: u32,
        palette_data: &[u8],
    ) -> Self {
        let dvo_depth = dim.ilog2() - 1;
        let render_pipeline = create_shader_pipeline(device, surface_config, dvo_depth);
        let compute_pipeline = create_compute_pipeline(device, dvo_depth);

        let camera_buffer = create_camera_buffer(device, camera_data);
        let dvo_buffer = create_dvo_buffer(device, dim);
        let palette_buffer = create_palette_buffer(device, palette_data);
        let vertex_buffer = create_vertex_buffer(device);
        let voxels_texture = create_voxels_texture(device, queue, dim, voxels_data);

        let camera_bind_group = create_camera_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(0),
            &camera_buffer,
        );
        let dvo_bind_group = create_dvo_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(1),
            &dvo_buffer,
            &palette_buffer,
        );
        let voxels_bind_group = create_voxels_bind_group(
            device,
            &compute_pipeline.get_bind_group_layout(0),
            &voxels_texture,
            &dvo_buffer,
        );

        Self {
            camera_buffer,
            dvo_buffer,
            palette_buffer,
            vertex_buffer,

            camera_bind_group,
            dvo_bind_group,
            voxels_bind_group,

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
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.dvo_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        render_pass.draw(0..6, 0..1);
    }

    pub(crate) fn compute(&self, encoder: &mut CommandEncoder, dim: u32) {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("compute pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.compute_pipeline);
        compute_pass.set_bind_group(0, &self.voxels_bind_group, &[]);

        let mut workgroups = dim / 8; // workgroup_size = (4, 4, 4)
        while workgroups > 0 {
            println!("compute {workgroups} workgroups");
            compute_pass.dispatch_workgroups(workgroups, workgroups, workgroups);
            workgroups /= 2;
        }
    }

    pub(crate) fn reload_shaders(
        &mut self,
        device: &Device,
        surface_config: &SurfaceConfiguration,
        dvo_depth: u32,
    ) {
        self.render_pipeline = create_shader_pipeline(device, surface_config, dvo_depth);
        self.compute_pipeline = create_compute_pipeline(device, dvo_depth);
    }
}

pub(crate) fn create_palette_buffer(device: &Device, palette_data: &[u8]) -> Buffer {
    let palette_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("palette buffer"),
        contents: palette_data,
        usage: BufferUsages::STORAGE,
    });

    palette_buffer
}

pub(crate) fn create_dvo_buffer(device: &Device, dim: u32) -> Buffer {
    // 4 bytes per dvo node (1 u32)
    let depth = dim.ilog2();
    let nodes = (8u64.pow(depth) - 1) / 7;
    println!(
        "dvo nodes: {nodes} ({}B = {} MiB)",
        nodes * 4,
        nodes * 4 / 1024 / 1024
    );

    let dvo_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("dvo buffer"),
        usage: BufferUsages::STORAGE,
        size: nodes * 4,
        mapped_at_creation: false,
    });

    dvo_buffer
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
        label: Some("Camera buffer"),
        contents: camera_data,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });

    camera_buffer
}

pub(crate) fn create_voxels_texture(
    device: &Device,
    queue: &Queue,
    dim: u32,
    voxels_data: &[u8],
) -> Texture {
    println!("compute texture: {}", voxels_data.len());
    let dvo_buffer = device.create_texture_with_data(
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
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        voxels_data,
    );

    dvo_buffer
}

pub(crate) fn create_camera_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    camera_buffer: &Buffer,
) -> BindGroup {
    let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Camera bind group"),
        layout: &bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });

    camera_bind_group
}

pub(crate) fn create_dvo_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    dvo_buffer: &Buffer,
    palette_buffer: &Buffer,
) -> BindGroup {
    let dvo_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("dvo bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: dvo_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: palette_buffer.as_entire_binding(),
            },
        ],
    });

    dvo_bind_group
}

pub(crate) fn create_voxels_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    voxels_texture: &Texture,
    dvo_buffer: &Buffer,
) -> BindGroup {
    let texture_view = voxels_texture.create_view(&TextureViewDescriptor {
        label: Some("voxels texture view"),
        format: None,
        dimension: None,
        aspect: TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: None,
        base_array_layer: 0,
        array_layer_count: None,
    });

    let dvo_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("dvo bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&texture_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: dvo_buffer.as_entire_binding(),
            },
        ],
    });

    dvo_bind_group
}

pub(crate) fn create_shader_pipeline(
    device: &Device,
    surface_config: &SurfaceConfiguration,
    dvo_depth: u32,
) -> RenderPipeline {
    let preproc_ctx = preproc::Context {
        main: "src/shader.wgsl".into(),
        constants: HashMap::from([("DVO_DEPTH".to_owned(), format!("{dvo_depth}u"))]),
    };
    let shader_source = build_shader(&preproc_ctx).unwrap();

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("shader"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });

    let dvo_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("dvo bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let camera_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Bind group layout"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("render pipeline layout"),
        bind_group_layouts: &[&camera_bind_group_layout, &dvo_bind_group_layout],
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

    render_pipeline
}

fn create_compute_pipeline(device: &Device, dvo_depth: u32) -> ComputePipeline {
    let preproc_ctx = preproc::Context {
        main: "src/compute.wgsl".into(),
        constants: HashMap::from([("DVO_DEPTH".to_owned(), format!("{dvo_depth}u"))]),
    };
    let shader_source = build_shader(&preproc_ctx).unwrap();

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("compute"),
        source: ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("compute bind group"),
        entries: &[
            BindGroupLayoutEntry {
                //in_tex
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Uint,
                    view_dimension: TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // dvo
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
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

    compute_pipeline
}
