//! # nexus-accel
//!
//! Automatic hardware dispatch for numeric work. The planner/runtime never
//! hand-writes threading or SIMD; this crate picks the execution strategy from
//! the workload size:
//!
//! * **Scalar** — tiny arrays, where vector/thread setup would dominate.
//! * **SIMD** — medium arrays, using portable 8-wide `f64` lanes.
//! * **SIMD + threads** — large arrays, fanned across all cores, each lane-wide.
//!
//! A pluggable [`Device`] abstraction leaves room for a GPU backend; today the
//! honest set of devices is CPU-only, and dispatch reports exactly which path
//! it chose so the choice is observable, not magic.
#![feature(portable_simd)]

use std::simd::num::SimdFloat;
use std::simd::Simd;
use std::thread;

const LANES: usize = 8;
type F64s = Simd<f64, LANES>;

/// The execution strategy chosen for a numeric workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Scalar,
    Simd,
    SimdThreads,
    Gpu,
}

impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::Scalar => "scalar",
            Backend::Simd => "simd",
            Backend::SimdThreads => "simd+threads",
            Backend::Gpu => "gpu",
        }
    }
}

/// Workloads at or above this size are eligible for GPU offload (below it, the
/// PCIe transfer cost outweighs the GPU's throughput).
#[allow(dead_code)] // referenced only on the `gpu` feature path
const GPU_MIN: usize = 1_000_000;

#[cfg(feature = "gpu")]
fn gpu_instance() -> Option<&'static nexus_gpu::Gpu> {
    use std::sync::OnceLock;
    static G: OnceLock<Option<nexus_gpu::Gpu>> = OnceLock::new();
    G.get_or_init(nexus_gpu::Gpu::new).as_ref()
}

/// The GPU adapter name, if a GPU backend is compiled in and available.
pub fn gpu_name() -> Option<String> {
    #[cfg(feature = "gpu")]
    {
        gpu_instance().map(|g| g.adapter_name().to_string())
    }
    #[cfg(not(feature = "gpu"))]
    {
        None
    }
}

/// Upper bound for single-dispatch GPU offload (keeps workgroup count within
/// portable limits at 256 threads/group); larger arrays use the CPU.
#[allow(dead_code)] // referenced only on the `gpu` feature path
const GPU_MAX: usize = 16_000_000;

/// GPU offload is **opt-in** (`NEXUS_GPU=1`). The GPU kernels compute in f32,
/// so silently routing f64 work there would quietly drop ~9 significant
/// digits; correctness wins by default and the speed trade is explicit.
fn gpu_opted_in() -> bool {
    matches!(std::env::var("NEXUS_GPU").as_deref(), Ok("1") | Ok("on") | Ok("force"))
}

#[cfg(feature = "gpu")]
fn gpu_ready(n: usize) -> bool {
    gpu_opted_in() && (GPU_MIN..=GPU_MAX).contains(&n) && gpu_instance().is_some()
}
#[cfg(not(feature = "gpu"))]
fn gpu_ready(_n: usize) -> bool {
    false
}

#[allow(dead_code)] // used only on the `gpu` feature path
fn to_f32(a: &[f64]) -> Vec<f32> {
    a.iter().map(|&x| x as f32).collect()
}
#[allow(dead_code)] // used only on the `gpu` feature path
fn to_f64(a: Vec<f32>) -> Vec<f64> {
    a.into_iter().map(|x| x as f64).collect()
}

/// A compute device. Only `Cpu` is compiled today; `Gpu` is reserved so the
/// dispatcher's interface is GPU-ready without pretending a backend exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    Cpu,
    Gpu,
}

/// Number of worker threads available (logical cores).
pub fn cores() -> usize {
    thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

/// SIMD lane width for `f64`.
pub fn simd_lanes() -> usize {
    LANES
}

/// Devices actually available for dispatch (honest: CPU only unless a real GPU
/// backend is compiled in).
pub fn available_devices() -> Vec<Device> {
    vec![Device::Cpu]
}

/// Cost-based backend selection from problem size (CPU tiers).
pub fn choose_backend(n: usize) -> Backend {
    if n < 64 {
        Backend::Scalar
    } else if n < 100_000 {
        Backend::Simd
    } else {
        Backend::SimdThreads
    }
}

/// The device + strategy a workload routes to. Large workloads route to the GPU
/// when a backend is compiled in and an adapter is present; otherwise to the
/// best CPU tier.
pub fn route(n: usize) -> (Device, Backend) {
    if gpu_ready(n) {
        (Device::Gpu, Backend::Gpu)
    } else {
        (Device::Cpu, choose_backend(n))
    }
}

// ---- elementwise binary ops (auto-dispatched) ---------------------------

fn binary_dispatch(a: &[f64], b: &[f64], f: fn(F64s, F64s) -> F64s, sf: fn(f64, f64) -> f64) -> Vec<f64> {
    assert_eq!(a.len(), b.len(), "vector length mismatch");
    let n = a.len();
    match choose_backend(n) {
        Backend::Scalar => a.iter().zip(b).map(|(x, y)| sf(*x, *y)).collect(),
        Backend::Simd => simd_binary(a, b, f, sf),
        _ => threaded_binary(a, b, f, sf),
    }
}

fn simd_binary(a: &[f64], b: &[f64], f: fn(F64s, F64s) -> F64s, sf: fn(f64, f64) -> f64) -> Vec<f64> {
    let n = a.len();
    let mut out = vec![0.0; n];
    let chunks = n / LANES;
    for i in 0..chunks {
        let lo = i * LANES;
        let va = F64s::from_slice(&a[lo..lo + LANES]);
        let vb = F64s::from_slice(&b[lo..lo + LANES]);
        f(va, vb).copy_to_slice(&mut out[lo..lo + LANES]);
    }
    for i in chunks * LANES..n {
        out[i] = sf(a[i], b[i]);
    }
    out
}

fn threaded_binary(a: &[f64], b: &[f64], f: fn(F64s, F64s) -> F64s, sf: fn(f64, f64) -> f64) -> Vec<f64> {
    let n = a.len();
    let nthreads = cores().min((n / 50_000).max(1));
    if nthreads <= 1 {
        return simd_binary(a, b, f, sf);
    }
    let mut out = vec![0.0; n];
    let chunk = n.div_ceil(nthreads);
    thread::scope(|s| {
        for (oc, (ac, bc)) in out
            .chunks_mut(chunk)
            .zip(a.chunks(chunk).zip(b.chunks(chunk)))
        {
            s.spawn(move || {
                let r = simd_binary(ac, bc, f, sf);
                oc.copy_from_slice(&r);
            });
        }
    });
    out
}

pub fn add(a: &[f64], b: &[f64]) -> Vec<f64> {
    #[cfg(feature = "gpu")]
    if gpu_ready(a.len()) {
        if let Some(g) = gpu_instance() {
            return to_f64(g.binary(&to_f32(a), &to_f32(b), nexus_gpu::Op::Add));
        }
    }
    binary_dispatch(a, b, |x, y| x + y, |x, y| x + y)
}
pub fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    #[cfg(feature = "gpu")]
    if gpu_ready(a.len()) {
        if let Some(g) = gpu_instance() {
            return to_f64(g.binary(&to_f32(a), &to_f32(b), nexus_gpu::Op::Sub));
        }
    }
    binary_dispatch(a, b, |x, y| x - y, |x, y| x - y)
}
pub fn mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    #[cfg(feature = "gpu")]
    if gpu_ready(a.len()) {
        if let Some(g) = gpu_instance() {
            return to_f64(g.binary(&to_f32(a), &to_f32(b), nexus_gpu::Op::Mul));
        }
    }
    binary_dispatch(a, b, |x, y| x * y, |x, y| x * y)
}

pub fn scale(a: &[f64], k: f64) -> Vec<f64> {
    let n = a.len();
    let ks = F64s::splat(k);
    let mut out = vec![0.0; n];
    let chunks = n / LANES;
    for i in 0..chunks {
        let lo = i * LANES;
        (F64s::from_slice(&a[lo..lo + LANES]) * ks).copy_to_slice(&mut out[lo..lo + LANES]);
    }
    for i in chunks * LANES..n {
        out[i] = a[i] * k;
    }
    out
}

// ---- reductions ---------------------------------------------------------

/// SIMD horizontal sum.
pub fn sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 64 {
        return a.iter().sum();
    }
    let chunks = n / LANES;
    let mut acc = F64s::splat(0.0);
    for i in 0..chunks {
        let lo = i * LANES;
        acc += F64s::from_slice(&a[lo..lo + LANES]);
    }
    let mut total = acc.reduce_sum();
    for &x in &a[chunks * LANES..n] {
        total += x;
    }
    total
}

/// SIMD/threaded dot product.
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    #[cfg(feature = "gpu")]
    {
        let n = a.len();
        if gpu_ready(n) {
            if let Some(g) = gpu_instance() {
                return g.dot(&to_f32(a), &to_f32(b)) as f64;
            }
        }
    }
    dot_cpu(a, b)
}

/// Dot product forced onto the CPU (SIMD/threads), never the GPU. Lets callers
/// compare CPU and GPU paths head-to-head.
pub fn dot_cpu(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let n = a.len();
    match choose_backend(n) {
        Backend::Scalar => a.iter().zip(b).map(|(x, y)| x * y).sum(),
        Backend::Simd => simd_dot(a, b),
        _ => {
            let nthreads = cores().min((n / 50_000).max(1));
            let chunk = n.div_ceil(nthreads);
            thread::scope(|s| {
                let handles: Vec<_> = a
                    .chunks(chunk)
                    .zip(b.chunks(chunk))
                    .map(|(ac, bc)| s.spawn(move || simd_dot(ac, bc)))
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).sum()
            })
        }
    }
}

fn simd_dot(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len();
    let chunks = n / LANES;
    let mut acc = F64s::splat(0.0);
    for i in 0..chunks {
        let lo = i * LANES;
        let va = F64s::from_slice(&a[lo..lo + LANES]);
        let vb = F64s::from_slice(&b[lo..lo + LANES]);
        acc += va * vb;
    }
    let mut total = acc.reduce_sum();
    for i in chunks * LANES..n {
        total += a[i] * b[i];
    }
    total
}

pub fn mean(a: &[f64]) -> f64 {
    if a.is_empty() {
        0.0
    } else {
        sum(a) / a.len() as f64
    }
}

pub fn max(a: &[f64]) -> f64 {
    a.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}
pub fn min(a: &[f64]) -> f64 {
    a.iter().copied().fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_matches_scalar_add() {
        let a: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..1000).map(|i| (i * 2) as f64).collect();
        let got = add(&a, &b);
        for i in 0..1000 {
            assert_eq!(got[i], a[i] + b[i]);
        }
    }

    #[test]
    fn sum_and_dot_correct() {
        let a: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        assert_eq!(sum(&a), 5050.0);
        let ones = vec![1.0; 100];
        assert_eq!(dot(&a, &ones), 5050.0);
        assert_eq!(dot(&a, &a), (1..=100).map(|i| (i * i) as f64).sum::<f64>());
    }

    #[test]
    fn threaded_path_matches_scalar() {
        // Large enough to trigger SimdThreads.
        let n = 250_000;
        let a: Vec<f64> = (0..n).map(|i| (i % 7) as f64).collect();
        let b: Vec<f64> = (0..n).map(|i| (i % 3) as f64).collect();
        assert_eq!(choose_backend(n), Backend::SimdThreads);
        let got = mul(&a, &b);
        for i in [0, 1, 100, n - 1] {
            assert_eq!(got[i], a[i] * b[i]);
        }
        // dot via threads vs scalar reference
        let scalar: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        assert!((dot(&a, &b) - scalar).abs() < 1e-6);
    }

    #[test]
    fn auto_dispatch_preserves_f64_precision() {
        // Without the NEXUS_GPU opt-in, big dots must stay on the f64 CPU path
        // bit-for-bit — no silent f32 downgrade inside the GPU size window.
        let n = 1_500_000;
        let a: Vec<f64> = (0..n).map(|i| i as f64).collect();
        assert_eq!(dot(&a, &a), dot_cpu(&a, &a));
        let (dev, _) = route(n);
        assert_eq!(dev, Device::Cpu);
    }

    #[test]
    fn backend_selection() {
        assert_eq!(choose_backend(10), Backend::Scalar);
        assert_eq!(choose_backend(5000), Backend::Simd);
        assert_eq!(choose_backend(500_000), Backend::SimdThreads);
    }
}
