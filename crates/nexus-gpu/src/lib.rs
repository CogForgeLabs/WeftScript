//! # nexus-gpu
//!
//! A real GPU compute backend built on `wgpu` (Vulkan/DX12/Metal). Elementwise
//! vector operations run as WGSL compute shaders on the physical GPU when an
//! adapter is present; callers fall back to the CPU otherwise. GPUs are f32-
//! native, so values are computed in single precision on-device.

use std::borrow::Cow;
use wgpu::util::DeviceExt;

/// Which elementwise kernel to run.
#[derive(Clone, Copy, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
}

impl Op {
    fn entry(&self) -> &'static str {
        match self {
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
        }
    }
}

const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> o: array<f32>;

@compute @workgroup_size(256)
fn add(@builtin(global_invocation_id) g: vec3<u32>) {
    let i = g.x;
    if (i < arrayLength(&o)) { o[i] = a[i] + b[i]; }
}
@compute @workgroup_size(256)
fn sub(@builtin(global_invocation_id) g: vec3<u32>) {
    let i = g.x;
    if (i < arrayLength(&o)) { o[i] = a[i] - b[i]; }
}
@compute @workgroup_size(256)
fn mul(@builtin(global_invocation_id) g: vec3<u32>) {
    let i = g.x;
    if (i < arrayLength(&o)) { o[i] = a[i] * b[i]; }
}
"#;

/// A compute-bound kernel: many FLOPs per element, where the GPU's thousands of
/// cores dominate a CPU. Each element runs `iters` iterations of a transcendental
/// update — the regime where GPU offload genuinely wins.
const HEAVY_SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read_write> o: array<f32>;
@group(0) @binding(2) var<uniform> iters: u32;

@compute @workgroup_size(256)
fn heavy(@builtin(global_invocation_id) g: vec3<u32>) {
    let i = g.x;
    if (i < arrayLength(&o)) {
        var x = a[i];
        for (var k = 0u; k < iters; k = k + 1u) {
            x = x * 1.0000001 + sin(x);
        }
        o[i] = x;
    }
}
"#;

/// A live GPU device with compiled compute pipelines.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    heavy_shader: wgpu::ShaderModule,
    name: String,
}

impl Gpu {
    /// Acquire a GPU. Returns `None` if no compute adapter is available.
    pub fn new() -> Option<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        let name = adapter.get_info().name;
        // Use the adapter's real limits so large dispatches (millions of
        // workgroups) are permitted on capable hardware.
        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("nexus-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .ok()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vecops"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });
        let heavy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("heavy"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(HEAVY_SHADER)),
        });
        Some(Gpu { device, queue, shader, heavy_shader, name })
    }

    /// The physical adapter name (e.g. "NVIDIA GeForce RTX 4060 Laptop GPU").
    pub fn adapter_name(&self) -> &str {
        &self.name
    }

    /// Elementwise op `a OP b` executed on the GPU.
    pub fn binary(&self, a: &[f32], b: &[f32], op: Op) -> Vec<f32> {
        assert_eq!(a.len(), b.len());
        let n = a.len();
        if n == 0 {
            return Vec::new();
        }
        let size = (n * std::mem::size_of::<f32>()) as u64;

        let buf_a = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("a"),
            contents: bytemuck::cast_slice(a),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let buf_b = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("b"),
            contents: bytemuck::cast_slice(b),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let buf_o = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("o"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("op"),
                layout: None,
                module: &self.shader,
                entry_point: op.entry(),
                compilation_options: Default::default(),
                cache: None,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf_a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: buf_b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: buf_o.as_entire_binding() },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let groups = (n as u32).div_ceil(256);
            cpass.dispatch_workgroups(groups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&buf_o, 0, &readback, 0, size);
        self.queue.submit(Some(encoder.finish()));

        // Map and read the result back.
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let data = slice.get_mapped_range();
        let out: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        out
    }

    /// Dot product: elementwise multiply on the GPU, reduce on the CPU.
    pub fn dot(&self, a: &[f32], b: &[f32]) -> f32 {
        self.binary(a, b, Op::Mul).iter().sum()
    }

    /// Compute-bound map: `iters` transcendental iterations per element. This is
    /// the workload class where GPU offload wins decisively.
    pub fn heavy(&self, a: &[f32], iters: u32) -> Vec<f32> {
        let n = a.len();
        if n == 0 {
            return Vec::new();
        }
        let size = (n * 4) as u64;
        let buf_a = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("a"),
            contents: bytemuck::cast_slice(a),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let buf_o = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("o"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let buf_iters = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("iters"),
            contents: bytemuck::cast_slice(&[iters]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rb"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("heavy"),
                layout: None,
                module: &self.heavy_shader,
                entry_point: "heavy",
                compilation_options: Default::default(),
                cache: None,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buf_a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: buf_o.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: buf_iters.as_entire_binding() },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups((n as u32).div_ceil(256), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&buf_o, 0, &readback, 0, size);
        self.queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let data = slice.get_mapped_range();
        let out: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_add_matches_cpu_when_available() {
        match Gpu::new() {
            None => {
                eprintln!("no GPU adapter; skipping");
            }
            Some(gpu) => {
                eprintln!("GPU: {}", gpu.adapter_name());
                let a: Vec<f32> = (0..1000).map(|i| i as f32).collect();
                let b: Vec<f32> = (0..1000).map(|i| (i * 3) as f32).collect();
                let got = gpu.binary(&a, &b, Op::Add);
                for i in 0..1000 {
                    assert_eq!(got[i], a[i] + b[i]);
                }
                let d = gpu.dot(&a, &b);
                let expect: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
                assert!((d - expect).abs() / expect.abs() < 1e-4);
            }
        }
    }
}
