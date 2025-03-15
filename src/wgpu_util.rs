use nalgebra_glm as glm;
use pollster::FutureExt;
use std::borrow::Cow;
use std::path::PathBuf;
use std::str::FromStr;
use wesl::syntax::{self};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::preproc::{self};

const DVO_FORMAT: TextureFormat = if cfg!(feature = "byte_voxels") {
    TextureFormat::R8Uint
} else {
    TextureFormat::R32Uint
};
// const DVO_FORMAT = TextureFormat::R8Uint;

fn dvo_format_to_string() -> String {
    match DVO_FORMAT {
        TextureFormat::R8Uint => "r8uint".to_string(),
        TextureFormat::R32Uint => "r32uint".to_string(),
        _ => unreachable!(),
    }
}

pub(crate) struct WgpuState {
    pub camera_buffer: Buffer,
    pub lights_buffer: Buffer,
    octree_texture: Texture,
    voxels_texture: Texture,
    colors_texture: Texture,
    vertex_buffer: Buffer,

    uniforms_bind_group: BindGroup,
    octree_bind_group: BindGroup,

    render_pipeline: RenderPipeline,
    octree_pipeline: ComputePipeline,
    mipmap_pipeline: ComputePipeline,
}

pub(crate) struct ShaderConstants {
    pub octree_depth: u32,
    pub svo_depth: u32,
    pub svo_max_iter: u32,
    pub dvo_depth: u32,
    pub dvo_max_iter: u32,
    pub grid_depth: u32,
    pub grid_max_iter: u32,
    pub shadow_max_iter: u32,
    pub shadow_cone_angle: u32,
    pub shadow_strength: u32,
    pub ao_strength: u32,
    pub msaa_level: u32,
    pub debug_display: u32,
}

pub(crate) struct Buffers<'a> {
    pub camera: &'a [u8],
    pub lights: &'a [u8],
    pub voxels: &'a [u8],
    pub colors: &'a [u8],
}

impl ShaderConstants {
    pub fn to_wesl(&self) -> syntax::TranslationUnit {
        fn decl(name: &str, expr: Expression) -> GlobalDeclaration {
            GlobalDeclaration::Declaration(Declaration {
                attributes: Vec::new(),
                kind: DeclarationKind::Const,
                ident: Ident::new(name.to_string()),
                ty: None,
                initializer: Some(expr.into()),
            })
        }
        use syntax::*;
        TranslationUnit {
            global_declarations: vec![
                decl(
                    "OCTREE_DEPTH",
                    Expression::Literal(LiteralExpression::U32(self.octree_depth)),
                ),
                decl(
                    "SVO_DEPTH",
                    Expression::Literal(LiteralExpression::U32(self.svo_depth)),
                ),
                decl(
                    "SVO_MAX_ITER",
                    Expression::Literal(LiteralExpression::U32(self.svo_max_iter)),
                ),
                decl(
                    "DVO_DEPTH",
                    Expression::Literal(LiteralExpression::U32(self.dvo_depth)),
                ),
                decl(
                    "DVO_MAX_ITER",
                    Expression::Literal(LiteralExpression::U32(self.dvo_max_iter)),
                ),
                decl(
                    "GRID_DEPTH",
                    Expression::Literal(LiteralExpression::U32(self.grid_depth)),
                ),
                decl(
                    "GRID_MAX_ITER",
                    Expression::Literal(LiteralExpression::U32(self.grid_max_iter)),
                ),
                decl(
                    "SHADOW_MAX_ITER",
                    Expression::Literal(LiteralExpression::U32(self.shadow_max_iter)),
                ),
                decl(
                    "SHADOW_CONE_ANGLE",
                    Expression::Literal(LiteralExpression::U32(self.shadow_cone_angle)),
                ),
                decl(
                    "SHADOW_STRENGTH",
                    Expression::Literal(LiteralExpression::U32(self.shadow_strength)),
                ),
                decl(
                    "AO_STRENGTH",
                    Expression::Literal(LiteralExpression::U32(self.ao_strength)),
                ),
                decl(
                    "MSAA_LEVEL",
                    Expression::Literal(LiteralExpression::U32(self.msaa_level)),
                ),
                decl(
                    "DEBUG_DISPLAY",
                    Expression::Literal(LiteralExpression::U32(self.debug_display)),
                ),
                GlobalDeclaration::TypeAlias(TypeAlias {
                    attributes: Vec::new(),
                    ident: Ident::new("texture_storage_read".to_string()),
                    ty: TypeExpression {
                        path: None,
                        ident: Ident::new("texture_storage_3d".to_string()),
                        template_args: Some(vec![
                            TemplateArg {
                                expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                    Ident::new(dvo_format_to_string()),
                                ))
                                .into(),
                            },
                            TemplateArg {
                                expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                    Ident::new("read".to_string()),
                                ))
                                .into(),
                            },
                        ]),
                    },
                }),
                GlobalDeclaration::TypeAlias(TypeAlias {
                    attributes: Vec::new(),
                    ident: Ident::new("texture_storage_write".to_string()),
                    ty: TypeExpression {
                        path: None,
                        ident: Ident::new("texture_storage_3d".to_string()),
                        template_args: Some(vec![
                            TemplateArg {
                                expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                    Ident::new(dvo_format_to_string()),
                                ))
                                .into(),
                            },
                            TemplateArg {
                                expression: Expression::TypeOrIdentifier(TypeExpression::new(
                                    Ident::new("write".to_string()),
                                ))
                                .into(),
                            },
                        ]),
                    },
                }),
            ],
            ..Default::default()
        }
    }
}

impl WgpuState {
    pub(crate) fn new(
        device: &Device,
        queue: &Queue,
        surface_config: &SurfaceConfiguration,
        buffers: &Buffers,
        constants: &ShaderConstants,
    ) -> Self {
        let dim = 2u32.pow(constants.octree_depth + 1);
        let render_pipeline = create_shader_pipeline(device, surface_config, constants);
        let octree_pipeline = create_octree_pipeline(device, constants).unwrap();
        let mipmap_pipeline = create_mipmap_pipeline(device, constants).unwrap();

        let camera_buffer = create_camera_buffer(device, buffers.camera);
        let lights_buffer = create_lights_buffer(device, buffers.lights);
        let svo_buffer = create_svo_buffer(device, dim);
        let dvo_texture = create_dvo_texture(device, dim);
        let colors_texture = create_colors_texture(device, queue, dim, buffers.colors);
        let vertex_buffer = create_vertex_buffer(device);
        let voxels_texture = create_voxels_texture(device, queue, dim, buffers.voxels);

        let uniforms_bind_group = create_uniforms_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(0),
            &camera_buffer,
            &lights_buffer,
        );
        let octree_bind_group = create_octree_bind_group(
            device,
            &render_pipeline.get_bind_group_layout(1),
            &svo_buffer,
            &dvo_texture,
            &colors_texture,
        );
        Self {
            camera_buffer,
            lights_buffer,
            octree_texture: dvo_texture,
            voxels_texture,
            colors_texture,
            vertex_buffer,

            uniforms_bind_group,
            octree_bind_group,

            render_pipeline,
            octree_pipeline,
            mipmap_pipeline,
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
        render_pass.set_bind_group(1, &self.octree_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..1);
    }

    pub(crate) fn compute_octree(
        &self,
        device: &Device,
        encoder: &mut CommandEncoder,
        mut dim: u32,
    ) {
        let mut depth = 0;

        fn compute_single_pass(
            pipeline: &ComputePipeline,
            device: &Device,
            encoder: &mut CommandEncoder,
            input_view: &TextureView,
            output_view: &TextureView,
            dim: u32,
        ) {
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("compute bind group"),
                layout: &pipeline.get_bind_group_layout(0),
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

            {
                let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("compute pass"),
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch_workgroups(dim / 2, dim / 2, dim / 2);
            }
        }

        // first pass
        {
            let input_view = self.voxels_texture.create_view(&TextureViewDescriptor {
                label: Some("input texture view"),
                ..Default::default()
            });

            let output_view = self.octree_texture.create_view(&TextureViewDescriptor {
                label: Some("output texture view"),
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            });

            println!("compute octree, depth={depth}, dim={dim}");
            compute_single_pass(
                &self.octree_pipeline,
                device,
                encoder,
                &input_view,
                &output_view,
                dim,
            );
            dim /= 2;
            depth += 1;
        }

        // next passes
        while dim > 1 {
            let input_view = self.octree_texture.create_view(&TextureViewDescriptor {
                label: Some("input texture view"),
                base_mip_level: depth - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let output_view = self.octree_texture.create_view(&TextureViewDescriptor {
                label: Some("output texture view"),
                base_mip_level: depth,
                mip_level_count: Some(1),
                ..Default::default()
            });

            println!("compute octree, depth={depth}, dim={dim}");
            compute_single_pass(
                &self.octree_pipeline,
                device,
                encoder,
                &input_view,
                &output_view,
                dim,
            );
            dim /= 2;
            depth += 1;
        }
    }

    pub(crate) fn compute_mipmap(
        &self,
        device: &Device,
        encoder: &mut CommandEncoder,
        mut dim: u32,
    ) {
        let mut depth = 0;

        while dim > 2 {
            let input_view = self.colors_texture.create_view(&TextureViewDescriptor {
                label: Some("input texture view"),
                base_mip_level: depth,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let output_view = self.colors_texture.create_view(&TextureViewDescriptor {
                label: Some("output texture view"),
                base_mip_level: depth + 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            println!("compute mipmap, depth={depth}, dim={dim}");
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("mipmap bind group"),
                layout: &self.mipmap_pipeline.get_bind_group_layout(0),
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

            {
                let mut render_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("mipmap pass"),
                    timestamp_writes: None,
                });

                render_pass.set_pipeline(&self.mipmap_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.dispatch_workgroups(dim / 2, dim / 2, dim / 2)
            }

            dim /= 2;
            depth += 1;
        }
    }

    pub(crate) fn reload_shaders(
        &mut self,
        device: &Device,
        surface_config: &SurfaceConfiguration,
        constants: &ShaderConstants,
    ) {
        self.render_pipeline = create_shader_pipeline(device, surface_config, constants);
        if let Some(octree_pipeline) = create_octree_pipeline(device, constants) {
            self.octree_pipeline = octree_pipeline;
        }
        if let Some(mipmap_pipeline) = create_mipmap_pipeline(device, constants) {
            self.mipmap_pipeline = mipmap_pipeline;
        }
    }
}

pub(crate) fn create_colors_texture(
    device: &Device,
    queue: &Queue,
    dim: u32,
    colors_data: &[u8],
) -> Texture {
    // let colors_texture = device.create_texture_with_data(
    //     queue,
    //     &TextureDescriptor {
    //         label: Some("colors texture"),
    //         size: Extent3d {
    //             width: dim,
    //             height: dim,
    //             depth_or_array_layers: dim,
    //         },
    //         mip_level_count: dim.ilog2(),
    //         sample_count: 1,
    //         dimension: TextureDimension::D3,
    //         format: TextureFormat::Rgba8Unorm,
    //         usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
    //         view_formats: &[],
    //     },
    //     util::TextureDataOrder::LayerMajor,
    //     colors_data,
    // );
    let size = Extent3d {
        width: dim,
        height: dim,
        depth_or_array_layers: dim,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("colors texture"),
        size,
        mip_level_count: dim.ilog2(),
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING
            | TextureUsages::STORAGE_BINDING
            | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let copy = ImageCopyTexture {
        texture: &texture,
        mip_level: 0,
        origin: Origin3d::ZERO,
        aspect: TextureAspect::All,
    };
    let layout = ImageDataLayout {
        offset: 0,
        bytes_per_row: Some(dim * 4),
        rows_per_image: Some(dim),
    };
    queue.write_texture(copy, colors_data, layout, size);

    texture
}

pub(crate) fn create_svo_buffer(device: &Device, _dim: u32) -> Buffer {
    let size_heuristic = 500 * 1024; // 500Mib
    let svo_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("svo buffer"),
        usage: BufferUsages::STORAGE,
        size: size_heuristic,
        mapped_at_creation: false,
    });

    svo_buffer
}

pub(crate) fn create_dvo_texture(device: &Device, dim: u32) -> Texture {
    let depth = dim.ilog2();

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
        format: DVO_FORMAT,
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
            format: DVO_FORMAT,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        },
        util::TextureDataOrder::LayerMajor,
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

pub(crate) fn create_octree_bind_group(
    device: &Device,
    bind_group_layout: &BindGroupLayout,
    svo_buffer: &Buffer,
    dvo_texture: &Texture,
    colors_texture: &Texture,
) -> BindGroup {
    let octree_view = dvo_texture.create_view(&TextureViewDescriptor {
        label: Some("octree texture view"),
        ..Default::default()
    });

    let colors_view = colors_texture.create_view(&TextureViewDescriptor {
        label: Some("colors texture view"),
        base_mip_level: 0,
        mip_level_count: Some(1),
        ..Default::default()
    });

    let linear_sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("linear sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToBorder,
        address_mode_v: AddressMode::ClampToBorder,
        address_mode_w: AddressMode::ClampToBorder,
        border_color: Some(SamplerBorderColor::TransparentBlack),
        ..Default::default()
    });

    let nearest_sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("nearest sampler"),
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToBorder,
        address_mode_v: AddressMode::ClampToBorder,
        address_mode_w: AddressMode::ClampToBorder,
        border_color: Some(SamplerBorderColor::TransparentBlack),
        ..Default::default()
    });

    let octree_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("octree bind group"),
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: svo_buffer.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&octree_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::TextureView(&colors_view),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Sampler(&linear_sampler),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Sampler(&nearest_sampler),
            },
        ],
    });

    octree_bind_group
}

fn fallback_shader() -> naga::Module {
    naga::front::wgsl::parse_str(include_str!("fallback.wgsl")).unwrap()
}

pub(crate) fn create_shader_pipeline(
    device: &Device,
    surface_config: &SurfaceConfiguration,
    constants: &ShaderConstants,
) -> RenderPipeline {
    let constants = constants.to_wesl();
    let preproc_ctx = preproc::Context {
        main: &PathBuf::from_str("src/shader.wgsl").unwrap(),
        constants: &constants,
    };
    let shader_module = match preproc::compile_shader(&preproc_ctx) {
        Ok(module) => module,
        Err(err) => {
            eprintln!("{err}");
            fallback_shader()
        }
    };

    device.push_error_scope(ErrorFilter::Validation);

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("shader"),
        source: ShaderSource::Naga(Cow::Owned(shader_module)),
    });

    let err = device.pop_error_scope().block_on();
    match err {
        Some(err) => {
            eprintln!("wgpu shader error: {err}");
            panic!();
        }
        None => println!("compiled render shader"),
    }

    let octree_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("octree bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                // svo
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
                // dvo
                binding: 1,
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
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // linear_sampler
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            BindGroupLayoutEntry {
                // nearest_sampler
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
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

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("render pipeline layout"),
        bind_group_layouts: &[&uniforms_bind_group_layout, &octree_bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("render pipeline"),
        layout: Some(&pipeline_layout),
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
            compilation_options: Default::default(),
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
            compilation_options: Default::default(),
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
        // cache: None,
    });

    pipeline
}

fn create_octree_pipeline(device: &Device, constants: &ShaderConstants) -> Option<ComputePipeline> {
    let constants = constants.to_wesl();
    let preproc_ctx = preproc::Context {
        main: &PathBuf::from_str("src/compute_octree.wgsl").unwrap(),
        constants: &constants,
    };

    let shader_module = match preproc::compile_shader(&preproc_ctx) {
        Ok(module) => module,
        Err(err) => {
            eprintln!("{err}");
            return None;
        }
    };

    device.push_error_scope(ErrorFilter::Validation);

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("compute"),
        source: ShaderSource::Naga(Cow::Owned(shader_module)),
    });

    let err = device.pop_error_scope().block_on();
    match err {
        Some(err) => {
            eprintln!("wgpu shader error: {err}");
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
                    format: DVO_FORMAT,
                    view_dimension: TextureViewDimension::D3,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // octree
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: DVO_FORMAT,
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
        compilation_options: Default::default(),
        // cache: None,
    });

    Some(compute_pipeline)
}

fn create_mipmap_pipeline(device: &Device, constants: &ShaderConstants) -> Option<ComputePipeline> {
    let constants = constants.to_wesl();
    let preproc_ctx = preproc::Context {
        main: &PathBuf::from_str("src/mipmap.wgsl").unwrap(),
        constants: &constants,
    };

    let shader_module = match preproc::compile_shader(&preproc_ctx) {
        Ok(module) => module,
        Err(err) => {
            eprintln!("{err}");
            return None;
        }
    };

    device.push_error_scope(ErrorFilter::Validation);

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("mipmap"),
        source: ShaderSource::Naga(Cow::Owned(shader_module)),
    });

    let err = device.pop_error_scope().block_on();
    match err {
        Some(err) => {
            eprintln!("wgpu shader error: {err}");
            return None;
        }
        None => println!("compiled compute shader"),
    }

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("mipmap bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                // in_tex
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::ReadOnly,
                    format: TextureFormat::Rgba8Unorm,
                    view_dimension: TextureViewDimension::D3,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                // out_tex
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::Rgba8Unorm,
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

    let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("mipmap pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "cs_main",
        compilation_options: Default::default(),
        // cache: None,
    });

    Some(pipeline)
}
