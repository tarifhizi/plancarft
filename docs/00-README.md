# Planets Craft — Engine Documentation

**Planets Craft** is a mobile-optimized procedural world generation engine for generating whole planets deterministically. This documentation covers the architecture, algorithms, and implementation details.

## Quick Start

**New to the project?** Start here:
1. [Architecture Overview](01-ARCHITECTURE-OVERVIEW.md) — Bird's-eye system design
2. [World Generation](02-WORLD-GENERATION.md) — Geometry, tectonics, elevation algorithms
3. [Climate & Biomes & Hydrology](03-CLIMATE-BIOMES-HYDROLOGY.md) — Weather, ecology, water systems

**Implementing rendering or integration?**
- [Rendering Pipeline](04-RENDERING-PIPELINE.md) — GPU architecture, shaders, LOD streaming
- [FFI & Integration](05-FFI-AND-INTEGRATION.md) — Rust ↔ C++ memory contract

**Optimizing or debugging?**
- [Performance & Debugging](06-PERFORMANCE-AND-DEBUGGING.md) — Profiling, targets, troubleshooting

---

## Project Info

| Property | Value |
|----------|-------|
| **Target Platform** | Mobile (Android/iOS) |
| **Core Languages** | Rust (simulation), C++ (rendering) |
| **Rendering APIs** | Vulkan (Android), Metal (iOS) |
| **Design Goal** | O(n) algorithms, minimal memory, deterministic output |
| **Data Structure** | Hex-sphere (10k–100k cells), Structure of Arrays (SoA) |

---

## Key Concepts

### Hex Sphere
- Start with icosahedron (12 vertices, 20 triangles)
- Subdivide recursively
- Relax vertices toward uniform distribution
- Convert to dual mesh → mostly hexagons with 12 pentagons

### Tectonic Plates
- Partition hex cells into plates
- Assign motion vectors per plate
- Classify boundaries (convergent/divergent/transform)
- Base elevation determined by plate type

### Multi-Octave Noise
- Combine large-scale (continents), mid-scale (mountains), small-scale (hills)
- Fractional Brownian Motion (FBM)
- Quantized to u16 for memory efficiency

### Climate → Biomes → Hydrology
1. **Temperature:** Latitude + elevation gradient
2. **Wind bands:** Hadley/Ferrel/Polar cells
3. **Precipitation:** Orographic lifting (rain shadows)
4. **Biomes:** 2D lookup (temperature × precipitation)
5. **Rivers:** Flow downhill; accumulate from sources

### Streaming LOD
- **Planet LOD:** Coarse (~10k cells); entire planet
- **Regional LOD:** Medium (~100k cells); camera approach area
- **Ground LOD:** Fine (heightmap tiles); player vicinity

---

## Architecture at a Glance

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
└──────────────��──────────────────────────┘
```

---

## Data Flow

```
Seed
  → Icosphere (12 verts, 20 faces)
    → Subdivide → Project to sphere → Relax (Lloyd)
      → Build hex mesh (dual)
        → Assign plates → Base elevation
          → FBM noise (3 octaves)
            → Smooth + normalize
              → Compute climate (temp, wind, precip)
                → Assign biomes
                  → Compute hydrology (flow, rivers, lakes)
                    → Export to C++ (SoA arrays)
                      → GPU upload (vertex/index buffers)
                        → Render LOD patches
```

---

## Glossary

| Term | Definition |
|------|-----------|
| **Cell** | Hexagonal (or pentagonal) region on sphere; unit of simulation |
| **Plate** | Tectonic plate; assigned motion vector; determines base elevation |
| **LOD** | Level of Detail; planet (coarse) → regional (medium) → ground (fine) |
| **Patch** | GPU-renderable cluster of hexes; ~1–4k cells per patch |
| **SoA** | Structure of Arrays; all elevations together, all temps together, etc. for cache efficiency |
| **FFI** | Foreign Function Interface; Rust-C++ boundary via raw pointers |
| **FBM** | Fractional Brownian Motion; multi-octave procedural noise |
| **CVT** | Centroidal Voronoi Tessellation; Lloyd relaxation approximation |

---

## Design Principles

1. **O(n) Complexity** — Every algorithm targets linear time complexity (or O(n log n) worst-case)
2. **SoA Data Layout** — Parallel arrays enable SIMD vectorization and cache efficiency
3. **Deterministic Output** — Same seed always produces identical planet
4. **Streaming Architecture** — LOD transitions hide generation latency
5. **Thin FFI** — Minimize Rust ↔ C++ communication overhead
6. **Quantized Types** — Use u16/u8 for storage; f32 only in shaders
7. **Mobile-First** — Every design decision considers thermal and power budget

---

## Documentation Files

| File | Purpose |
|------|---------|
| `00-README.md` | You are here. Overview and navigation. |
| `01-ARCHITECTURE-OVERVIEW.md` | System design, glossary, data flow diagram. |
| `02-WORLD-GENERATION.md` | Geometry, tectonics, elevation with pseudocode and test cases. |
| `03-CLIMATE-BIOMES-HYDROLOGY.md` | Climate, biome classification, hydrology systems. |
| `04-RENDERING-PIPELINE.md` | GPU architecture, shaders, LOD streaming. |
| `05-FFI-AND-INTEGRATION.md` | Rust ↔ C++ contract, memory ownership, error handling. |
| `06-PERFORMANCE-AND-DEBUGGING.md` | Performance targets, profiling, troubleshooting. |

---

## Recommended Reading Order

**For world builders / game designers:**
1. 00-README (you are here)
2. 01-ARCHITECTURE-OVERVIEW
3. 02-WORLD-GENERATION (sections 1–2)
4. 03-CLIMATE-BIOMES-HYDROLOGY (sections 1–2)

**For rendering engineers:**
1. 00-README (you are here)
2. 01-ARCHITECTURE-OVERVIEW
3. 04-RENDERING-PIPELINE
4. 05-FFI-AND-INTEGRATION

**For systems engineers / performance optimization:**
1. 01-ARCHITECTURE-OVERVIEW
2. 06-PERFORMANCE-AND-DEBUGGING
3. 04-RENDERING-PIPELINE (GPU section)

**For new team members:**
1. All of the above in order

---

## Version & Maintenance

| Property | Value |
|----------|-------|
| **Documentation Version** | 1.1 |
| **Last Updated** | 2026-07-04 |
| **Maintainer** | Tarif Hizi |

---

## External References

- **Rendering:** Vulkan 1.2 (Android), Metal 3.0 (iOS)
- **Parallelism:** Rayon crate for work-stealing scheduling
- **Noise:** Rust SIMD-friendly FBM implementation
- **Serialization:** Bincode or MessagePack for planet snapshots
- **Math:** Custom SIMD-friendly vectors (no external linear algebra dependencies)
