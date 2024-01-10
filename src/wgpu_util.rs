use wgpu::util::DeviceExt;
use wgpu::*;

pub(crate) fn init_voxels_buffers(
    device: &Device,
    voxels_data: &[u8],
    palette_data: &[u8],
) -> (Buffer, Buffer, BindGroup, BindGroupLayout) {
    let voxels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("voxels buffer"),
        contents: voxels_data,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let palette_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("palette buffer"),
        contents: palette_data,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let voxels_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxels bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

    let voxels_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("voxels bind group"),
        layout: &voxels_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: voxels_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: palette_buffer.as_entire_binding(),
            },
        ],
    });

    (
        voxels_buffer,
        palette_buffer,
        voxels_bind_group,
        voxels_bind_group_layout,
    )
}

pub(crate) fn init_camera_buffers(
    device: &Device,
    camera_data: &[u8],
) -> (Buffer, BindGroup, BindGroupLayout) {
    let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Camera buffer"),
        contents: camera_data,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let camera_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Camera bind group"),
        layout: &camera_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: camera_buffer.as_entire_binding(),
        }],
    });

    (camera_buffer, camera_bind_group, camera_bind_group_layout)
}
