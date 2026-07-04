# Architecture Overview

## System Diagram

```
┌─────────────────────────────────────────┐
│         Rust Subsystem (World Gen)      │
│  - Geometry (icosphere → hex mesh)      │
│  - Tectonics (plates + boundaries)      │
│  - Elevation (FBM + continent mask)     │
│  - Climate (temperature, wind, precip)  │
│  - Hydrology (flow, rivers, lakes)      │
│  - Biomes (classification)              │
└──────────────┬──────────────────────────┘
               │ (FFI: raw pointers)
               ▼
┌─────────────────────────────────────────┐
│      C++ Subsystem (Rendering)          │
│  - GPU device management                │
│  - Mesh batching & culling              │
│  - Command buffers (Vulkan/Metal)       │
│  - Shader pipelines                     │
│  - LOD streaming                        │
└─────────────────────────────────────────┘
```

---

## Subsystem Breakdown

### 1.1 Rust Subsystem — World Simulation

**Responsibilities:**
- Geometry generation (icosphere → hex sphere)
- Tectonic simulation (plates, boundaries, base elevation)
- Elevation generation (FBM + continent mask)
- Climate simulation (temperature, wind, precipitation)
- Hydrology (flow direction, rivers, lakes)
- Biome classification (2D lookup table)
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
- GPU memory allocation & streaming
- Mesh batching & frustum culling
- Command buffer orchestration
- Shader pipelines (vertex, fragment, tessellation)
- Texture/buffer updates
- LOD switching & patch loading/unloading

**Why C++:**
- ✅ Absolute control over GPU resources
- ✅ Mature Vulkan/Metal ecosystem
- ✅ Predictable performance on mobile
- ✅ Low-level hardware access

---

## Data Flow Pipeline

```
Input: Seed (u64)
  ↓
[1] Icosphere Generation
    - 12 canonical vertices
    - 20 triangular faces
    - Output: vertices[], faces[]
  ↓
[2] Subdivision
    - Recursively replace triangles with 4
    - N iterations → 20 × 4^N triangles
    - Output: refined vertices[], faces[]
  ↓
[3] Spherical Projection
    - Normalize all vertices to radius R
    - Output: normalized vertices[]
  ↓
[4] Lloyd Relaxation
    - Move vertices toward centroids
    - ~5–10 iterations
    - Output: relaxed vertices[]
  ↓
[5] Dual Mesh (Hex Sphere)
    - Compute triangle centroids
    - Connect adjacent centroids
    - Output: cells[] (mostly hexagons + 12 pentagons)
  ↓
[6] Tectonic Simulation
    - Seed plates via flood-fill
    - Assign motion vectors
    - Classify boundaries (convergent/divergent/transform)
    - Output: plate_id[], base_elevation[]
  ↓
[7] Elevation Refinement (FBM)
    - Multi-octave Fractional Brownian Motion
    - Blend tectonics (70%) + noise (30%)
    - Output: elevation[] (quantized to u16)
  ↓
[8] Normalization & Sea Level
    - Normalize elevations to [0, 1]
    - Mark ~25% of cells as ocean
    - Output: elevation[], is_ocean[]
  ↓
[9] Climate Simulation
    - Compute temperature (latitude + altitude)
    - Compute wind bands (Hadley/Ferrel/Polar)
    - Compute precipitation (orographic)
    - Output: temperature[], wind[], precipitation[]
  ↓
[10] Biome Classification
    - Lookup table: (temperature, precipitation) → biome
    - Override for altitude (alpine) and ocean
    - Output: biome[]
  ↓
[11] Hydrology Simulation
    - Flow direction: each cell → lowest neighbor
    - Flow accumulation: upstream count per cell
    - River detection: threshold on accumulation
    - Lake detection: sink cells (no lower neighbor)
    - Output: flow_to[], flow_accum[], is_river[], is_lake[]
  ↓
[12] Export to C++ (FFI)
    - Create PlanetData struct with raw pointers
    - Pass vertex positions, normals, indices
    - Output: PlanetData* (C++ owns reference)
  ↓
[13] GPU Upload (C++)
    - Create vertex buffer (vertex_count × 28 bytes)
    - Create index buffer (index_count × 2 bytes)
    - Create SSBO for dynamic elevation updates
    - Output: GPU resources
  ↓
[14] Render LOD Patches
    - For each patch in frustum:
      - Update elevation SSBO if LOD changed
      - Record draw command
      - Submit to GPU
    - Output: rendered frame
```

---

## Design Principles

### 1. O(n) Complexity First
Every algorithm targets linear time complexity:
- Geometry: O(n) subdivisions, relaxation
- Tectonics: O(n) flood-fill, O(n) boundary classification
- Elevation: O(n) FBM, O(n) smoothing
- Climate: O(n) temperature, O(n) wind, O(n) precipitation
- Hydrology: O(n) flow direction, O(n) flow accumulation

**Why:** Mobile CPUs are constrained; O(n log n) or worse causes thermal throttling.

### 2. SoA for SIMD
All data stored in Structure of Arrays format:

```rust
struct WorldData {
    elevation: Vec<u16>,      // All elevations together
    temperature: Vec<u8>,     // All temps together
    moisture: Vec<u8>,        // All moisture together
    biome: Vec<u8>,           // All biomes together
    plate_id: Vec<u16>,       // All plate IDs together
    flow_dir: Vec<u8>,        // All flow directions together
    flow_accum: Vec<u16>,     // All accumulations together
}
```

**Benefits:**
- ✅ Perfect alignment for SIMD operations (all elevations in one vector)
- ✅ Optimal cache-line streaming (load one field across many cells)
- ✅ True O(n) passes (no cache misses jumping between fields)

### 3. Thin FFI Boundary
Rust and C++ communicate only via raw pointers and simple structs:

```c
struct PlanetData {
    float* elevation_ptr;
    float* temperature_ptr;
    // ... more fields
    uint32_t cell_count;
};
```

**Why:** Minimize serialization, copying, and overhead.

### 4. Deterministic Output
Same seed always produces identical planet:
- All randomness seeded from input
- No floating-point comparison quirks (quantize to u16/u8 early)
- Reproducible for testing and streaming

### 5. Mobile-First Design
Every decision considers mobile constraints:
- Quantized types (u8, u16) instead of f32 arrays
- No expensive allocations in hot loops
- Streaming LOD (load on demand, unload out of view)
- Thermal budget: <2% CPU per frame

### 6. Streaming Architecture
LOD transitions hide latency:
- **Planet LOD:** Whole planet, coarse detail
- **Regional LOD:** Medium detail, loaded as camera approaches
- **Ground LOD:** Fine detail, only near player
- Old patches freed when out of view

### 7. Immutable Input
Geometry computed once, streamed to GPU:
- No dynamic re-meshing during gameplay
- Vertex/index buffers uploaded once and reused
- Only dynamic data: elevation offsets, biome indices, moisture/temperature values

---

## Data Model — Structure of Arrays

### Cell Data

```rust
struct Cell {
    position: Vec3,           // Centroid on sphere
    neighbors: [u32; 6],      // Indices (pentagons have 5)
    elevation: u16,           // Normalized height
    temperature: u8,          // Coarse temperature
    moisture: u8,             // Coarse moisture
    biome: u8,                // Enum index
    plate_id: u16,            // Tectonic plate
    is_ocean: bool,           // Derived from sea level
    flow_to: u8,              // Neighbor index for flow
    flow_accum: u16,          // Upstream count
}
```

**Memory per cell:** ~35 bytes (optimized for u16/u8 types)

### Plate Data

```rust
struct Plate {
    id: u16,
    motion: Vec2,             // Tangent motion vector
    is_continental: bool,     // Determines base elevation
}
```

---

## Complexity Analysis

| Stage | Algorithm | Complexity | Notes |
|-------|-----------|-----------|-------|
| Icosphere | Fixed | O(1) | 12 vertices, 20 faces always |
| Subdivision | Recursive | O(4^N) where N=levels | Pre-computed, not in hot path |
| Projection | Vectorized | O(n) | SIMD-friendly normalization |
| Relaxation | Lloyd | O(n × iterations) | ~5–10 iterations (linear in practice) |
| Hex mesh | Dual mesh | O(n) | Single pass over triangles |
| Tectonics | Flood-fill | O(n) | Linear scan + visited set |
| Base elevation | Lookup | O(n) | Simple rule per plate |
| FBM noise | SIMD | O(n × octaves) | ~4–6 octaves, vectorized |
| Climate | Lookup | O(n) | Temperature, wind, precipitation |
| Biomes | Lookup table | O(n) | 2D table lookup per cell |
| Hydrology | DAG pass | O(n) | Single topological sort |
| **Total** | | **O(n)** | Fully parallelizable with Rayon |

---

## Memory Layout

### Compact Types Strategy

| Data | Type | Bytes | Rationale |
|------|------|-------|-----------|
| elevation | u16 | 2 | Range [0, 65535] → normalized to [0, 1] |
| temperature | u8 | 1 | Range [-50, 60]°C → quantized |
| moisture | u8 | 1 | Range [0, 100]% → quantized |
| biome | u8 | 1 | 8 biome types (2^8 = 256) |
| plate_id | u16 | 2 | Up to 65k plates (overkill, but safe) |
| flow_dir | u8 | 1 | 6 neighbors (fits in 3 bits) |
| flow_accum | u16 | 2 | Up to 65k upstream cells |

**Total per cell (SoA):** ~80 KB per 10k cells, ~800 KB per 100k cells

**GPU memory:** ~1–7 MB (depending on LOD)

---

## FFI Contract

### Memory Ownership

**Rust owns all data.** C++ holds non-owning references:

1. Rust generates world and allocates SoA arrays
2. Rust passes raw pointers to C++ via FFI
3. C++ reads and uploads to GPU
4. C++ **must not** deallocate or resize
5. Rust may only modify between LOD transitions

### Safety Guarantees

**Rust promises:**
- All pointers valid until `free_planet()` called
- All data initialized (no undefined values)
- Memory layout matches C expectations (`#[repr(C)]`)

**C++ promises:**
- No dereferencing after `free_planet()`
- No writing to data (read-only)
- No alignment assumptions beyond `repr(C)`

---

## Glossary

| Term | Definition |
|------|-----------|
| **Cell** | Hexagonal (or pentagonal) region on sphere; unit of simulation |
| **Plate** | Tectonic plate; assigned motion vector; base elevation rule |
| **LOD** | Level of Detail; planet (coarse) → regional (medium) → ground (fine) |
| **Patch** | GPU-renderable cluster of hexes; ~1–4k cells per patch |
| **SoA** | Structure of Arrays; all elevations together, all temps together, etc. |
| **FFI** | Foreign Function Interface; Rust-C++ boundary via raw pointers |
| **FBM** | Fractional Brownian Motion; multi-octave procedural noise |
| **CVT** | Centroidal Voronoi Tessellation; Lloyd relaxation goal |
| **Convergent** | Plate boundary where plates collide (compression, mountains) |
| **Divergent** | Plate boundary where plates spread (spreading, rifts) |
| **Transform** | Plate boundary where plates slide parallel (shear) |

---

## Next Steps

- [World Generation →](02-WORLD-GENERATION.md) Detailed algorithms with pseudocode
- [Rendering Pipeline →](04-RENDERING-PIPELINE.md) GPU architecture and shaders
- [FFI Integration →](05-FFI-AND-INTEGRATION.md) Rust ↔ C++ contract details
