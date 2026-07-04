# Procedural Planet Engine — Complete Technical Architecture

**Version:** 1.0  
**Target Platform:** Mobile (Android/iOS)  
**Core Languages:** Rust (world simulation), C++ (Vulkan/Metal renderer)  
**Rendering APIs:** Vulkan (Android), Metal (iOS)  
**Data Model:** Hex-dominant sphere (10k–100k cells), SoA, quantized types  
**Performance Goal:** O(n) passes, minimal memory footprint, minimal thermal load

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Data Model](#data-model)
3. [World Generation Pipeline](#world-generation-pipeline)
4. [FFI Layer](#ffi-layer)
5. [Rendering Architecture](#rendering-architecture)
6. [Shader Architecture](#shader-architecture)
7. [Hydrology System](#hydrology-system)
8. [LOD Strategy](#lod-strategy)
9. [Performance Considerations](#performance-considerations)
10. [Project Structure](#project-structure)
11. [Build Pipeline](#build-pipeline)
12. [Future Extensions](#future-extensions)

---

## System Overview

The engine is divided into two primary subsystems working in concert:

### 1.1 Rust Subsystem — World Simulation

**Responsibilities:**
- Geometry generation
- Tectonics
- Elevation (FBM)
- Climate simulation
- Hydrology
- Biome classification
- Serialization & data compression

**Why Rust:**
- ✅ Memory safety without GC
- ✅ Zero-cost abstractions
- ✅ Native SIMD support
- ✅ High-performance parallelism (Rayon)
- ✅ Perfect fit for SoA numeric pipelines

### 1.2 C++ Subsystem — Rendering Backend

**Responsibilities:**
- Vulkan/Metal device management
- GPU memory allocation
- Mesh batching
- Command buffer orchestration
- Shader pipelines
- Texture/buffer updates
- Frustum culling
- LOD switching

**Why C++:**
- ✅ Absolute control over GPU resources
- ✅ Mature Vulkan/Metal ecosystem
- ✅ Predictable performance on mobile
- ✅ Low-level hardware access

---

## Data Model

### 2.1 Cell Structure (Structure of Arrays)

Each world cell is represented across multiple parallel arrays for maximum cache efficiency:

```rust
pub struct WorldData {
    pub elevation: Vec<u16>,
    pub temperature: Vec<u8>,
    pub moisture: Vec<u8>,
    pub biome: Vec<u8>,
    pub plate_id: Vec<u16>,
    pub flow_dir: Vec<u8>,
    pub flow_accum: Vec<u16>,
}
```

**Why SoA Layout:**
- ✅ Perfect alignment for SIMD operations
- ✅ Optimal cache-line streaming
- ✅ Enables true O(n) passes
- ✅ Ideal for mobile CPU constraints

### 2.2 Geometry Representation

```rust
pub struct Geometry {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub cell_to_vertex: Vec<[u32; 6]>,
    pub neighbors: Vec<[u32; 6]>,
}
```

---

## World Generation Pipeline

All stages are strictly **O(n)** and parallelized using Rayon for multi-core efficiency.

### 3.1 Geometry Generation

**Steps:**
1. Start from canonical icosahedron
2. Subdivide faces recursively
3. Project vertices to sphere
4. Relax vertices (Lloyd + Laplacian hybrid)
5. Generate dual mesh → hexagonal sphere

**Output:**
- ~10k–100k hexagonal cells
- 12 pentagons (icosahedron property)
- Uniform distribution across sphere

### 3.2 Tectonic Simulation

**Algorithm:**
- Seed plates via flood-fill
- Assign motion vectors
- Classify boundaries (convergent/divergent/transform)
- Assign base elevation by plate type

**Complexity:**
- O(n) flood-fill
- O(n) boundary classification

**Features:**
- Continental plates elevated above oceanic plates
- Convergent boundaries → mountain chains
- Divergent boundaries → rifts

### 3.3 Elevation Generation (FBM)

**Implementation:**
- SIMD-accelerated Fractional Brownian Motion
- 4–6 octaves of noise
- Quantized to `u16` early for memory efficiency

**Formula (Planet LOD):**
```
elevation = 0.70 × tectonics
          + 0.30 × continent_mask
          + 0.15 × noise_large
          + 0.10 × noise_mid
          + 0.05 × noise_small
```

**Continent Mask:**
- Low-frequency FBM field defines 2–6 major continents
- Smoothstep thresholds prevent fragmented islands
- Creates coherent landmasses and deep oceans

### 3.4 Climate Simulation

**Components:**
- Latitudinal temperature gradient
- Lapse rate (temperature decrease with altitude)
- Wind band formation
- Orographic precipitation (rain shadow effects)

**Implementation:**
- All passes are O(n)
- SoA layout for temperature/moisture arrays
- Temperature and moisture tint applied at render time

### 3.5 Hydrology System

#### Planet LOD – Macro Hydrology

**Flow Algorithm:**
1. River sources spawn at high-elevation tectonic ridges
2. Each cell flows to its lowest neighbor (O(n) complexity)
3. Each cell tracks upstream flow count (accumulation)
4. Rivers widen as accumulation increases

**Data Structures:**
- `flow_dir: u8` — direction to lowest neighbor
- `flow_accum: u16` — upstream cell count

**Features:**
- Continental interiors form closed basins → lakes and inland seas
- Rivers terminate at continental shelves, reinforcing coastlines
- Natural drainage patterns from tectonic structure

#### Regional LOD – Mid-Scale Hydrology

- **River meandering:** Mid-frequency noise perturbs flow paths
- **Lake formation:** Depressions below sea level accumulate water until overflow
- **Wetlands:** Flat lowlands near rivers flagged as marsh/swamp biomes

#### Ground LOD – Fine Hydrology

- **Riverbeds:** Local erosion simulation creates believable channels
- **Coastal detail:** Small estuaries and deltas at river mouths
- **Groundwater:** Per-cell moisture model supports vegetation placement

### 3.6 Biome Classification

**Inputs:**
- Temperature
- Moisture
- Elevation
- Flow accumulation

**Output:**
- `biome: u8` — biome type enumeration

---

## FFI Layer

The FFI boundary is intentionally **thin and explicit** to minimize overhead and complexity.

### 4.1 Rust Exports to C++

```rust
#[repr(C)]
pub struct PlanetBuffers {
    pub elevation_ptr: *const u16,
    pub biome_ptr: *const u8,
    pub moisture_ptr: *const u8,
    pub temperature_ptr: *const u8,
    pub len: u32,
}
```

**Flow:**
1. Rust completes world generation pipeline
2. Exports raw pointers to SoA arrays
3. C++ receives pointers and uploads to GPU buffers
4. Rust retains ownership; C++ holds non-owning references

---

## Rendering Architecture

### 5.1 Chunked Mesh System

The sphere is divided into **patches** for optimal batching and culling:

**Configuration:**
- **20–60 patches** per planet
- **1–4k hexes per patch**
- **One draw call per patch**

**Benefits:**
- ✅ Perfect batching granularity
- ✅ Frustum culling at patch level
- ✅ Minimal CPU overhead per frame
- ✅ Cache-friendly vertex streaming

### 5.2 Vertex Format (Interleaved)

```cpp
struct Vertex {
    half normal[3];           // 6 bytes
    float position[3];        // 12 bytes
    uint8_t biome;            // 1 byte
    uint16_t elevation_offset;// 2 bytes
    uint8_t color[4];         // 4 bytes
};
// Total: 25 bytes per vertex (packed)
```

**Design Rationale:**
- Interleaved layout improves cache coherency
- Half-precision normals save bandwidth
- Elevation stored as offset for LOD support
- Per-vertex biome index enables texture lookup

### 5.3 GPU Data Flow

**Static Data (uploaded once):**
- Vertex buffer
- Index buffer
- Patch metadata

**Dynamic Data (updated per frame):**
- SSBO/TBO for elevation offsets
- Biome indices
- Moisture/temperature values

**Update Strategy:**
- Persistent mapped buffers for reduced latency
- Triple buffering for concurrent GPU/CPU writes
- Only update changed chunks (dirty-flag tracking)

### 5.4 Command Buffer Strategy

#### Vulkan
- Primary command buffer per frame
- Secondary command buffers per chunk
- Reuse secondary buffers when static
- Reduces CPU submit overhead

#### Metal
- One render pass per frame
- One draw call per chunk
- Optimized for Apple GPUs

---

## Shader Architecture

### 6.1 Vertex Shader

**Responsibilities:**
- Apply elevation offset from SSBO
- Compute final world position
- Pass biome index to fragment shader
- Pass texture coordinates for detail mapping

### 6.2 Fragment Shader

**Responsibilities:**
- Sample biome LUT (color palette)
- Apply lighting (Phong/PBR)
- Apply moisture/temperature tint (for visual feedback)
- Sample detail normal maps at Ground LOD
- Blend between LOD levels for smooth transitions

---

## LOD Strategy

### Level 1: Planet LOD – Continent-First Elevation

**Goal:** Establish macro-scale geography

**Key Systems:**
- Tectonic plates establish continental structure
- Continent mask (low-frequency FBM) defines 2–6 landmasses
- Elevation blends tectonics (70%) with continent mask (30%)
- Hydrology computed at macro scale for river/coast placement
- Biome assignment follows continental zones

**Output:**
- Coarse elevation grid
- Continent-aligned river network
- Basin/lake zones
- Continental shelf definition

### Level 2: Regional LOD – Mid-Scale Geography

**Goal:** Add detail within continental framework

**Key Systems:**
- Mountain ranges and plateaus (mid-frequency noise)
- Valley systems following regional drainage
- Refined hydrology with river meandering
- Detailed biome transitions
- Lake formation in depressions

**Output:**
- Medium-resolution elevation
- River paths with variation
- Wetland and marsh zones
- Regional biome clusters

### Level 3: Ground LOD – Fine Detail

**Goal:** Maximize visual fidelity near player

**Key Systems:**
- Micro-terrain (rocks, vegetation, soil)
- High-frequency noise for surface variation
- Local erosion simulation (riverbeds, coastlines)
- Detail streaming (only in player vicinity)
- Estuaries and deltas at river mouths

**Output:**
- High-resolution elevation mesh
- Erosion-sculpted terrain features
- Detailed coastlines
- Ready for vegetation/entity placement

---

## Performance Considerations

### 7.1 CPU Performance

| Metric | Target |
|--------|--------|
| Worldgen Complexity | O(n) per stage |
| Parallelization | Rayon (all cores) |
| SIMD Utilization | FBM, climate, hydrology |
| Heap Allocations | None in hot loops |
| Frame Time Budget | <16ms for LOD switches |

### 7.2 GPU Performance

| Metric | Target |
|--------|--------|
| Draw Calls | 20–60 per frame |
| Vertex Buffer Format | Interleaved, half-precision normals |
| Index Size | 16-bit (supports up to 65k vertices/patch) |
| Overdraw | 0% (no transparent blending) |
| Mesh Rebuild | Static vertex/index buffers |
| Dynamic Updates | SSBO/TBO only |

### 7.3 Memory Footprint

| Component | Size (10k cells) | Size (100k cells) |
|-----------|-----------------|------------------|
| Elevation (u16) | 20 KB | 200 KB |
| Temperature (u8) | 10 KB | 100 KB |
| Moisture (u8) | 10 KB | 100 KB |
| Biome (u8) | 10 KB | 100 KB |
| Flow Dir (u8) | 10 KB | 100 KB |
| Flow Accum (u16) | 20 KB | 200 KB |
| **Total SoA Data** | **80 KB** | **800 KB** |
| GPU Vertex Buffer | ~500 KB | ~5 MB |
| GPU Index Buffer | ~200 KB | ~2 MB |
| **Total GPU Memory** | **~700 KB** | **~7 MB** |

**Mobile Target:** <50 MB for complete planet + resources

---

## Project Structure

```
/engine
├── /rust_core
│   ├── geometry/
│   │   ├── icosphere.rs
│   │   ├── hexmesh.rs
│   │   └── relaxation.rs
│   ├── tectonics/
│   │   ├── plates.rs
│   │   └── boundaries.rs
│   ├── elevation/
│   │   ├── fbm.rs
│   │   └── continent_mask.rs
│   ├── climate/
│   │   ├── temperature.rs
│   │   ├── wind.rs
│   │   └── precipitation.rs
│   ├── hydrology/
│   │   ├── flow.rs
│   │   ├── accumulation.rs
│   │   └── lakes.rs
│   ├── biomes/
│   │   └── classification.rs
│   └── ffi/
│       └── exports.rs
├── /cpp_render
│   ├── /vulkan
│   │   ├── device.cpp
│   │   ├── pipeline.cpp
│   │   └── command_buffer.cpp
│   ├── /metal
│   │   ├── device.mm
│   │   ├── pipeline.mm
│   │   └── command_buffer.mm
│   ├── /shaders
│   │   ├── terrain.vert
│   │   ├── terrain.frag
│   │   └── terrain.metal
│   ├── /mesh
│   │   ├── patch.cpp
│   │   └── batching.cpp
│   └── /gpu_buffers
│       ├── vertex_buffer.cpp
│       └── ssbo.cpp
├── /common
│   ├── math/
│   │   └── vector.h
│   └── serialization/
│       └── planet.rs
└── CMakeLists.txt
```

---

## Build Pipeline

### Rust Build

```bash
cargo build --release
```

**Configuration:**
- Workspace-based monorepo
- Link-Time Optimization (LTO) enabled
- Native CPU features enabled (`-C target-cpu=native`)
- Platform-specific builds for Android/iOS

### C++ Build

```bash
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
```

**Configuration:**
- CMake orchestration
- Separate Vulkan target (Android)
- Separate Metal target (iOS)
- Shader compilation:
  - GLSL via `glslang` (Vulkan)
  - MSL via Metal compiler (iOS)

---

## Future Extensions

### Phase 2
- [ ] GPU-accelerated climate simulation (compute shaders)
- [ ] Compute-based hydrology (parallel flow simulation)
- [ ] Real-time terrain deformation
- [ ] Procedural vegetation placement

### Phase 3
- [ ] Multi-resolution LOD sphere (cascading detail)
- [ ] Ocean wave simulation
- [ ] Weather systems (storms, wind patterns)
- [ ] Erosion-based terrain sculpting

### Phase 4
- [ ] Dynamic planet deformation (earthquakes, volcano)
- [ ] Tectonic animation over time
- [ ] Biome migration simulation
- [ ] Persistent world serialization/loading

---

## Design Principles

1. **O(n) Complexity First:** Every algorithm targets linear time complexity
2. **SoA for SIMD:** Data layout enables vectorization
3. **Thin FFI:** Minimize Rust ↔ C++ boundary overhead
4. **Mobile-First:** Every design decision considers thermal/power budget
5. **Streaming Architecture:** LOD transitions hide loading
6. **Immutable Input:** Geometry computed once, streamed to GPU
7. **Quantized Types:** u16/u8 over f32 where precision allows

---

## References & Standards

- **Rendering:** Vulkan 1.2 (Android), Metal 3.0 (iOS)
- **Parallelism:** Rayon crate for work-stealing scheduling
- **SIMD:** Packed_simd (stable Rust) + manual AVX2 intrinsics
- **Serialization:** Bincode or MessagePack for planet snapshots
- **Math:** Custom SIMD-friendly vectors (no dependency on nalgebra)

---

**Last Updated:** 2026-07-04  
**Maintainer:** Tarif Hizi
