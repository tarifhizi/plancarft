# Performance & Debugging

Profiling, optimization strategies, common bottlenecks, and debugging techniques.

---

## Profiling Strategy

### 1.1 CPU Profiling (Rust Side)

**Tools:**
- `perf` (Linux/Android)
- `Instruments` (macOS/iOS)
- `criterion` (benchmarking framework)

**Key metrics to track:**

```rust
use std::time::Instant;

fn profile_world_generation(seed: u64, cell_count: u32) {
    let start = Instant::now();
    
    let planet = PlanetData::generate(seed, cell_count, 12);
    
    let elapsed = start.elapsed();
    println!("Total generation: {:?}", elapsed);
    
    // Break down by stage
    println!("  - Geometry: {:?}", planet.timing.geometry_ms);
    println!("  - Tectonics: {:?}", planet.timing.tectonics_ms);
    println!("  - Elevation: {:?}", planet.timing.elevation_ms);
    println!("  - Climate: {:?}", planet.timing.climate_ms);
    println!("  - Hydrology: {:?}", planet.timing.hydrology_ms);
}
```

**Timing structure in PlanetData:**

```rust
#[repr(C)]
pub struct GenerationTimings {
    pub geometry_ms: u32,
    pub tectonics_ms: u32,
    pub elevation_ms: u32,
    pub climate_ms: u32,
    pub hydrology_ms: u32,
    pub total_ms: u32,
}
```

---

### 1.2 GPU Profiling (C++ Side)

**Vulkan timestamp queries:**

```cpp
class GPUProfiler {
private:
    vk::QueryPool query_pool_;
    std::vector<uint64_t> timestamps_;
    
public:
    void begin_query(vk::CommandBuffer cmd, uint32_t query_idx) {
        cmd.writeTimestamp(vk::PipelineStageFlagBits::eTopOfPipe, query_pool_, query_idx);
    }
    
    void end_query(vk::CommandBuffer cmd, uint32_t query_idx) {
        cmd.writeTimestamp(vk::PipelineStageFlagBits::eBottomOfPipe, query_pool_, query_idx);
    }
    
    uint64_t get_elapsed_ns(uint32_t start_idx, uint32_t end_idx) {
        vk::PhysicalDeviceProperties props = physical_device_.getProperties();
        uint64_t timestamp_period_ns = props.limits.timestampPeriod;
        
        uint64_t delta = timestamps_[end_idx] - timestamps_[start_idx];
        return delta * timestamp_period_ns;
    }
};
```

**Usage:**

```cpp
void render_frame_profiled(vk::CommandBuffer cmd, const PlanetEngine& engine) {
    GPUProfiler profiler;
    
    profiler.begin_query(cmd, 0);
    
    // Draw calls here
    for (const auto& patch : visible_patches) {
        cmd.drawIndexed(...);
    }
    
    profiler.end_query(cmd, 1);
    
    uint64_t gpu_time_ns = profiler.get_elapsed_ns(0, 1);
    std::cout << "GPU time: " << gpu_time_ns / 1e6 << " ms" << std::endl;
}
```

---

### 1.3 Memory Profiling

**Rust side (valgrind):**

```bash
# Generate debug binary
cargo build --release

# Run with valgrind
valgrind --tool=massif ./target/release/plancraft_gen --seed 12345 --cells 100000

# View results
ms_print massif.out.12345
```

**Expected memory usage:**
- 100k cells: ~2–3 MB (intermediate buffers)
- 1M cells: ~20–30 MB
- Peak during FBM generation (temporary allocations)

**C++ side (Android Profiler):**

```cpp
// In C++ rendering loop
#include <android/native_window.h>

void profile_gpu_memory() {
    vk::MemoryAllocateInfo alloc_info;
    alloc_info.allocationSize = vertex_buffer_size;
    
    vk::MemoryPropertyFlags props = vk::MemoryPropertyFlagBits::eDeviceLocal;
    
    vk::DeviceMemory memory = device_.allocateMemory(alloc_info);
    
    // Query allocation size
    vk::MemoryHeapProperties heap = physical_device_.getMemoryProperties();
    std::cout << "GPU memory allocated: " << alloc_info.allocationSize / (1024 * 1024) << " MB" << std::endl;
}
```

---

## Optimization Strategies

### 2.1 CPU-Side Optimizations

#### SIMD Vectorization

Use Rayon for parallel FBM computation:

```rust
use rayon::prelude::*;

fn fbm_noise_parallel(positions: &[Vec3], octaves: u32) -> Vec<f32> {
    positions.par_iter()
        .map(|&pos| fbm_noise(pos, octaves, 0.01, 2.0, 0.5))
        .collect()
}
```

**Expected speedup:** 3–4x on quad-core mobile CPU

#### Memory Layout Optimization

Verify SoA packing efficiency:

```rust
fn verify_memory_layout() {
    let mut cells = vec![0u32; 1_000_000];
    
    // SoA: all elevations together
    let elevations: Vec<u16> = (0..cells.len()).map(|_| 0).collect();
    let temperatures: Vec<u8> = (0..cells.len()).map(|_| 0).collect();
    
    // Benchmark: iterate over one field
    let start = Instant::now();
    let sum: u64 = elevations.iter().map(|&e| e as u64).sum();
    let elapsed = start.elapsed();
    
    println!("SoA iteration: {:?} (sum: {})", elapsed, sum);
    
    // vs AoS (bad)
    struct Cell {
        elevation: u16,
        temperature: u8,
    }
    let aos_cells: Vec<Cell> = (0..1_000_000)
        .map(|_| Cell { elevation: 0, temperature: 0 })
        .collect();
    
    let start = Instant::now();
    let sum: u64 = aos_cells.iter().map(|c| c.elevation as u64).sum();
    let elapsed = start.elapsed();
    
    println!("AoS iteration: {:?} (sum: {})", elapsed, sum);
}
```

**Expected result:** SoA ~2–3x faster

#### Quantization Benefits

Use u8/u16 instead of f32:

```rust
// Before: 4 bytes per value
let temps_f32: Vec<f32> = (0..1_000_000).map(|_| 25.5).collect();
let bytes_f32 = temps_f32.len() * 4;

// After: 1 byte per value
let temps_u8: Vec<u8> = (0..1_000_000).map(|_| 75).collect();
let bytes_u8 = temps_u8.len() * 1;

println!("Memory saved: {} MB", (bytes_f32 - bytes_u8) / (1024 * 1024));
// Output: Memory saved: 3 MB per 1M cells
```

---

### 2.2 GPU-Side Optimizations

#### Batch Optimization

Minimize draw calls by batching patches:

```cpp
void optimize_draw_calls(std::vector<Patch>& patches, vk::CommandBuffer cmd) {
    // Sort by LOD level
    std::sort(patches.begin(), patches.end(), [](const Patch& a, const Patch& b) {
        return a.lod < b.lod;
    });
    
    uint32_t draw_call_count = 0;
    
    for (const auto& patch : patches) {
        if (camera_frustum.intersects(patch.bounds)) {
            cmd.drawIndexed(patch.index_count, 1, patch.index_offset, patch.vertex_offset, 0);
            draw_call_count++;
        }
    }
    
    std::cout << "Draw calls: " << draw_call_count << std::endl;
}

// Target: <60 draw calls per frame
```

#### Frustum Culling

Implement camera-space sphere culling:

```cpp
bool frustum_intersects_sphere(const Frustum& frustum, const Sphere& sphere) {
    for (int i = 0; i < 6; ++i) {  // 6 frustum planes
        float dist = frustum.planes[i].distance_to_point(sphere.center);
        if (dist < -sphere.radius) {
            return false;  // Sphere is outside this plane
        }
    }
    return true;
}
```

**Expected result:** 50–70% culling rate on 60 patches

#### Persistent-Mapped Buffers

For LOD updates without stalls:

```cpp
vk::BufferCreateInfo buffer_info;
buffer_info.size = elevation_buffer_size;
buffer_info.usage = vk::BufferUsageFlagBits::eStorageBuffer;
buffer_info.sharingMode = vk::SharingMode::eExclusive;

vk::MemoryAllocateInfo alloc_info;
alloc_info.allocationSize = elevation_buffer_size;
alloc_info.memoryTypeIndex = find_persistent_memory_type();

// Map once at creation
float* persistent_ptr = (float*)device_.mapMemory(memory, 0, elevation_buffer_size);

// Update asynchronously
std::memcpy(persistent_ptr, new_elevations, elevation_buffer_size);

// No explicit flush (WC buffer)
```

---

## Common Bottlenecks

### 3.1 CPU Bottlenecks

| Bottleneck | Symptom | Solution |
|-----------|---------|----------|
| FBM noise computation | 60–80% CPU time | Use lookup tables; reduce octaves |
| Flow accumulation (topological sort) | O(n) with high constant | Parallelize with bucket sort |
| Climate simulation (orographic) | Wind advection loops | Cache wind vectors; skip distant cells |
| Biome lookup table | Repeated quantization | Pre-quantize; batch lookups |

**Profiling example:**

```rust
fn find_bottleneck() {
    let mut timings = HashMap::new();
    
    timings.insert("fbm", measure(|| fbm_computation()));
    timings.insert("flow", measure(|| flow_accumulation()));
    timings.insert("climate", measure(|| climate_simulation()));
    timings.insert("biome", measure(|| biome_classification()));
    
    for (stage, ms) in timings.iter() {
        println!("{}: {} ms", stage, ms);
    }
}
```

---

### 3.2 GPU Bottlenecks

| Bottleneck | Symptom | Solution |
|-----------|---------|----------|
| High draw calls (>100/frame) | GPU idle; CPU starved | Batch patches; increase patch size |
| Texture cache misses | Low FPS on large worlds | Compress biome palette; use mip-mapping |
| Vertex buffer thrashing | Frame stutters | Persistent-mapped buffers; async updates |
| Large index buffers (>100M) | Memory limited | LOD reduction; streaming |

**Detection:**

```cpp
void detect_gpu_bottleneck() {
    GPUProfiler profiler;
    
    if (profiler.draw_calls > 100) {
        std::cout << "CPU-limited: reduce draw calls" << std::endl;
    } else if (profiler.gpu_time_ms > 16.0) {
        std::cout << "GPU-limited: optimize shaders or reduce geometry" << std::endl;
    }
}
```

---

## Debugging Techniques

### 4.1 Validation Layers

**Enable Vulkan validation on Android:**

```cpp
#include <vulkan/vulkan.h>

std::vector<const char*> get_validation_layers() {
#ifdef NDEBUG
    return {};
#else
    return {"VK_LAYER_KHRONOS_validation"};
#endif
}

vk::InstanceCreateInfo instance_info;
instance_info.enabledLayerCount = validation_layers.size();
instance_info.ppEnabledLayerNames = validation_layers.data();

vk::Instance instance = vk::createInstance(instance_info);
```

---

### 4.2 RenderDoc Debugging

**Capture frame on Android:**

```bash
# Install RenderDoc on PC
# Connect Android device via USB debugging

# In app, trigger frame capture
adb shell setprop debug.vulkan.renderdoc 1

# Run app, capture, analyze
```

**Inside captured frame:**
- Inspect texture bindings
- Verify buffer contents
- Step through shader execution
- Check draw order

---

### 4.3 Assertion & Logging

**Rust side:**

```rust
#[cfg(debug_assertions)]
fn assert_valid_planet(planet: &PlanetData) {
    assert!(!planet.elevation_ptr.is_null(), "Elevation pointer null");
    assert_ne!(planet.cell_count, 0, "Zero cells");
    assert!(planet.cell_count < 10_000_000, "Unrealistic cell count");
    
    // Spot-check arrays
    unsafe {
        let elevations = std::slice::from_raw_parts(
            planet.elevation_ptr,
            planet.cell_count as usize,
        );
        for &e in elevations.iter().take(100) {
            assert!(e <= 65535, "Invalid elevation: {}", e);
        }
    }
}
```

**C++ side:**

```cpp
#ifdef DEBUG
#define GPU_CHECK(x) \
    if (!(x)) { \
        std::cerr << "GPU check failed: " #x << " at " << __FILE__ << ":" << __LINE__ << std::endl; \
        std::abort(); \
    }
#else
#define GPU_CHECK(x)
#endif

void validate_gpu_buffers(const PlanetEngine& engine) {
    GPU_CHECK(engine.is_valid());
    GPU_CHECK(engine.cell_count() > 0);
    GPU_CHECK(engine.cell_count() < 10000000);
}
```

---

### 4.4 Visualization Modes

**Toggle debug visualization in shader:**

```glsl
#ifdef DEBUG_MODE
    // Show flow accumulation as heat map
    out_color = vec4(flow_accum_normalized, 0.0, 1.0 - flow_accum_normalized, 1.0);
#endif
```

**Runtime toggle:**

```cpp
bool debug_mode = false;

void toggle_debug_visualization() {
    debug_mode = !debug_mode;
    update_shader_defines(debug_mode ? "DEBUG_MODE" : "");
    recompile_pipeline();
}
```

---

## Performance Targets

### 5.1 CPU Targets

| Stage | 10k Cells | 100k Cells | 1M Cells |
|-------|-----------|-----------|----------|
| Geometry | <1 ms | <5 ms | <50 ms |
| Tectonics | <2 ms | <10 ms | <100 ms |
| Elevation (FBM) | <5 ms | <50 ms | <500 ms |
| Climate | <3 ms | <20 ms | <200 ms |
| Hydrology | <2 ms | <15 ms | <150 ms |
| **Total** | **<13 ms** | **<100 ms** | **<1000 ms** |

**Target:** Generation completes in <100 ms for 100k cells on Snapdragon 888.

---

### 5.2 GPU Targets

| Metric | Target |
|--------|--------|
| Frame time | 16.7 ms (60 FPS) |
| Draw calls | <60 |
| GPU memory | <50 MB |
| Vertex throughput | >100M verts/sec |
| Texture bandwidth | <500 MB/sec |

**Example frame budget:**
- Culling: 2 ms
- GPU upload: 1 ms
- Draw calls: 12 ms
- Buffer sync: 1 ms
- **Total:** 16 ms (60 FPS target met)

---

### 5.3 Memory Targets

| Component | Budget |
|-----------|--------|
| Rust world data (100k cells) | <5 MB |
| GPU vertex/index buffers | <20 MB |
| GPU textures & SSBOs | <20 MB |
| C++ structures & overhead | <5 MB |
| **Total** | **<50 MB** |

---

## Benchmark Suite

### 6.1 Criterion Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_fbm(c: &mut Criterion) {
    c.bench_function("fbm_1m_cells", |b| {
        let positions: Vec<Vec3> = (0..1_000_000)
            .map(|i| Vec3::new(i as f32 % 100.0, (i as f32 / 100.0) % 100.0, 0.0))
            .collect();
        
        b.iter(|| {
            positions.iter()
                .map(|&p| fbm_noise(black_box(p), 4, 0.01, 2.0, 0.5))
                .collect::<Vec<_>>()
        });
    });
}

fn benchmark_flow_accumulation(c: &mut Criterion) {
    c.bench_function("flow_accum_1m_cells", |b| {
        let flow_to = vec![0u8; 1_000_000];
        
        b.iter(|| compute_flow_accumulation(black_box(&flow_to)));
    });
}

criterion_group!(benches, benchmark_fbm, benchmark_flow_accumulation);
criterion_main!(benches);
```

**Run:**

```bash
cargo bench
```

---

## Integration Checklist

- [ ] CPU profiling setup (perf/Instruments)
- [ ] GPU profiling setup (RenderDoc/timestamp queries)
- [ ] Memory profiling baseline established
- [ ] SIMD optimizations verified
- [ ] Draw call count <60 per frame
- [ ] Frustum culling implemented
- [ ] Persistent-mapped buffers tested
- [ ] Validation layers enabled (debug)
- [ ] Benchmark suite created
- [ ] Performance targets met (100 ms generation, 60 FPS rendering)

---

## Next Steps

- [README →](../README.md) Project overview
- [Architecture →](01-ARCHITECTURE-OVERVIEW.md) System design review
