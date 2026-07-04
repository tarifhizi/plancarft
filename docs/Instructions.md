# Procedural Worldbuilding Engine

Mobile — Optimized Hex–Sphere Planet Generator

A performance-oriented world generation pipeline designed for mobile devices. Uses an icosahedron → subdivision → dual-hex approach as the foundation for tectonics, elevation, climate, biomes, and hydrology.

---

## Table of contents

- Overview
- Goals & Constraints
- High-level Architecture
- Geometry pipeline
  - Icosahedron
  - Subdivision & Projection
  - Relaxation & Dual mesh
- Data model (with code blocks)
- Tectonic simulation
- Elevation generation
- Climate model
- Biome assignment
- Hydrology
- Mesh integrity
- Procedural Planet — Multi-Scale LOD Architecture
  - LOD0 (planet)
  - LOD1–3 (regional)
  - LOD4+ (ground)
- Rust/C++ technical notes
- Performance & memory considerations
- Full generation pipeline summary

---

## Overview

This document describes a compact, deterministic pipeline for generating whole planets on constrained devices. It emphasizes O(n) algorithms, small per-cell memory, and techniques that avoid expensive physics while producing believable large- and small-scale features.

## Goals & Constraints

- Target: mobile (limited CPU, RAM, battery)
- Topology: hex-dominant sphere (12 pentagons)
- Design goals:
  - O(n) passes over cells where possible
  - Avoid heavy physics (no full fluid simulation)
  - Minimize stored data per cell
  - Deterministic and reproducible algorithms

## High-level Architecture

Pipeline layers (each depends on previous):

1. Geometry: icosahedron → subdivided sphere → hex sphere
2. Tectonics: plates, boundaries, base elevation
3. Elevation: multi-octave noise + light smoothing
4. Climate: temperature, wind, precipitation
5. Biomes: classification by climate + elevation
6. Hydrology: flow directions, rivers, lakes

---

## Geometry pipeline

### Icosahedron generation

- Create 12 canonical vertices and 20 triangular faces
- Normalize vertices to radius R

### Subdivision

- For each triangle compute midpoints and replace with 4 triangles
- Repeat N times to reach target base cell count

### Spherical projection

- Project vertices to sphere:

```text
v = R * normalize(v)
```

### Relaxation (Lloyd / CVT)

- Move each vertex toward centroid of its neighboring cell centroids and reproject to sphere
- Repeat ~5–10 iterations (or fewer using CVT / Laplacian constrained smoothing)
- Notes: hybrid approaches (few Lloyd iterations + constrained Laplacian or quasi-Newton) often yield good quality with fewer iterations

### Dual mesh → Hex sphere

- Compute triangle centroids and connect centroids of adjacent triangles
- Result: mostly hexagonal cells with 12 pentagons

---

## Data model

Recommended compact Cell/Plate structures (SoA preferred for performance):

```c
struct Cell {
    Vector3 position;    // centroid on sphere
    int neighbors[6];    // indices into Cell array (pentagons will have one neighbor slot unused or duplicated)
    float elevation;     // normalized height
    float temperature;   // coarse
    float precipitation; // coarse
    int biome;           // enum index
    int plate_id;        // tectonic plate assignment
    bool is_ocean;       // derived from sea level
    int flow_to;         // neighbor index for flow direction
    int flow_accum;      // flow accumulation count
};

struct Plate {
    int id;
    Vector2 motion;      // planar motion vector on sphere tangent
    bool is_continental; // flag for base elevation / thickness
};
```

Implementation note: store arrays as Vec<u16>/<u8> where appropriate for memory efficiency (see Rust Core section).

---

## Tectonic simulation

1. Plate seeding
   - Pick P deterministic cells as seeds and flood-fill to assign plate_id
2. Plate motion
   - Assign a motion vector (2D tangent) per plate
3. Boundary classification
   - Compare motion vectors across shared edges and classify boundaries as convergent, divergent, transform
4. Base elevation
   - Continental plates → higher baseline
   - Oceanic plates → lower baseline
   - Add mountain ranges near convergent boundaries and rifts at divergent boundaries

Notes: keep this simulation light — no full mantle dynamics. Use motion vectors and local rules to derive collisional uplift/rift baselines.

---

## Elevation generation

Approach: combine tectonic base with multi-scale FBM noise and a few smoothing passes.

- Noise fields (FBM):
  - Large-scale: continents/plate-scale variation
  - Mid-scale: mountain chains and ranges
  - Small-scale: hills and terrain detail

Equation (per-cell):

```text
elevation = e_tect + w1 * noise_large + w2 * noise_mid + w3 * noise_small
```

- Sea level: normalize elevation distribution and mark cells with elevation <= sea_level as ocean
- Smoothing: 1–2 lightweight passes (Laplacian or neighbor-weighted average)
- Mountains: primarily generated near convergent boundaries; optionally generate ridgelines and apply simple erosion approximations for visual realism

---

## Climate model

### Temperature

Compute coarse temperature per cell using latitude and elevation:

```text
T = T_equator - |lat| * deltaT - elevation * lapse_rate
```

### Wind bands

- Implement banded global wind model (Hadley/Ferrel/Polar cells):
  - Equatorial: strong east-west
  - Subtropical: west-east
  - Mid-latitude: mixed
  - Polar: weak

### Precipitation

- Moisture sourced from oceans
- Advect moisture along wind directions
- Orographic precipitation: rain when wind is forced upward by terrain
- Create rain shadows on lee sides of mountain ranges

Implementation note: keep climate calculations coarse and O(n) — do not simulate full fluid dynamics. Use discrete advection along neighbor links or precomputed wind vectors.

---

## Biome assignment

Assign biomes based on temperature, precipitation, and elevation. Example candidates:

- Tundra
- Boreal forest
- Temperate forest
- Grassland
- Desert
- Savanna
- Rainforest

Define a simple 2D decision map (temperature vs precipitation) with elevation masks (high elevation → alpine/tundra).

---

## Hydrology

- Flow direction: each land cell flows to its lowest neighbor (tie-break deterministically)
- Flow accumulation: count upstream cells by a single downstream DAG pass
- Rivers: threshold the flow accumulation to mark river cells
- Lakes: detect cells with no lower neighbor (basin sinks)

Pseudocode (flow accumulation):

```text
for each cell c:
  flow_to[c] = index_of_lowest_neighbor(c)
  flow_accum[flow_to[c]] += 1 (propagate upstream in topological order)
```

Keep this single-pass or O(n log n) at worst — avoid iterative hydraulic simulations.

---

## Mesh Integrity

Common issues:
- Subdivision mismatches
- Indexing / floating-point duplication errors
- Projection rounding
- LOD seam gaps

Solutions:
- Vertex snapping: round projected coordinates to tolerance (e.g., 1e-6) before deduplication
- Edge registry: maintain a hash map of edges during subdivision to reuse shared vertices
- Seam correction pass: stitch/merge duplicate vertices at LOD borders
- Debug: wireframe rendering with color-coded unshared edges and logging of vertex counts pre/post dedup

Implementation notes:
- Deduplicate immediately after projection
- Maintain consistent edge registry across LOD levels
- Always run seam correction when building LOD borders

---

## Procedural Planet — Multi-Scale LOD Architecture

Three layers:

### 1) Planet-Scale LOD (LOD0)
- Purpose: whole-planet low-resolution representation; stable base for higher LODs
- Geometry: subdivided icosahedron → hex/pent cells (~10k–100k cells)
- Data: low-frequency elevation, plate_id, coarse moisture/temperature, coarse biome
- Algorithms: tectonics, global noise, climate bands

### 2) Regional LOD (LOD1–3)
- Purpose: medium detail where camera approaches
- Subdivision per parent hex: LOD1=7 cells, LOD2=19 cells, LOD3=37 cells
- Data: medium-frequency elevation, local climate, river flow, local biomes
- Algorithms: multi-octave noise refinement, local erosion approximations, flow/rivers, climate downscaling

### 3) Ground LOD (LOD4+)
- Purpose: high-res terrain for ground exploration
- Approach: convert regional hex patch → heightmap tile (128×128 or 256×256)
- Geometry: GPU tessellated heightmap
- Data: micro-noise, material masks, object spawn maps
- Algorithms: heightmap extraction, micro-noise layering, procedural object placement

---

## Rust Core — Data Model & Pipeline Notes

- Use Structure of Arrays (SoA) for cache-friendly access
- Planet data compact types:
  - elevation: Vec<u16>
  - temperature: Vec<u8>
  - moisture: Vec<u8>
  - plate_id: Vec<u16>
  - biome: Vec<u8>
  - flow_dir: Vec<u8>
  - flow_accum: Vec<u16>

Pipeline:
- LOD0: build hex-sphere, run tectonics, generate low-frequency elevation, compute global climate
- LOD1–3: triggered by camera, subdivide parent hex, refine elevation, compute rivers, downscale climate
- LOD4+: extract heightmap, apply micro-noise, generate masks, spawn objects

---

## C++ Rendering Pipeline

- Geometry:
  - Static planet mesh for LOD0
  - Dynamic patch meshes for LOD1–3
  - Heightmap-based terrain for LOD4+
- GPU data: SSBO/TBO, persistent mapped buffers, triple buffering for streaming
- Shaders:
  - Vertex: hex-sphere deformation
  - Tessellation: ground LOD refinement
  - Fragment: biome-based material blending

Streaming flow:
- Camera requests LOD
- Rust generates data asynchronously
- C++ uploads to GPU
- Old patches freed when out of range

---

## Performance & memory considerations

- Use compact types (int16, uint8) for large arrays
- Avoid complex erosion / fluid sims on mobile
- Keep algorithms O(n) and deterministic
- Stream and free LOD patches aggressively

---

## Full generation pipeline summary

1. Build icosahedron
2. Subdivide
3. Project to sphere
4. Relax vertices (Lloyd/CVT)
5. Build hex sphere (dual mesh)
6. Generate tectonic plates
7. Compute base elevation from plates
8. Add noise layers (multi-scale)
9. Normalize elevation + set sea level
10. Compute temperature
11. Compute wind + precipitation
12. Assign biomes
13. Compute rivers + lakes

---

If you'd like, I can:
- Split this into multiple Markdown files (geometry.md, tectonics.md, LOD.md)
- Add diagrams or ASCII art
- Convert pseudocode into Rust/C++ examples

Tell me which changes you'd like next and I'll update the repo.
