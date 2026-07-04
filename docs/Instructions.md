---
title: Instructions
---

🌍 Procedural Worldbuilding Engine

Mobile — Optimized Hex — Sphere Planet Generator

A complete, detailed description of a performance —oriented world generation pipeline designed for mobile devices, using an icosahedron → subdivision → hex — sphere approach as the[...]

Table of contents

Goals and constraints

High —level architecture

Geometry pipeline

Data model

Tectonic simulation

Elevation generation

Climate model

Biome assignment

Hydrology

Mesh Integrity

Procedural Planet – Multi‑Scale LOD Architecture

Performance and memory considerations

Full generation pipeline summary

Goals and constraints

Target platform: mobile (limited CPU, RAM, and battery).

Topology: hex —dominant sphere (with 12 pentagons).

Design goals:

O(n) passes over cells wherever possible.

Avoid heavy physics (no full fluid sim, no complex erosion).

Minimize stored data per cell.

Keep algorithms deterministic and reproducible.

High —level architecture

The world is generated in layers, each depending on the previous:

Geometry: icosahedron → subdivided sphere → hex sphere.

Tectonics: plates, boundaries, base elevation.

Elevation: refine heightmap with noise and smoothing.

Climate: temperature, wind, precipitation.

Biomes: classify cells into ecosystems.

Hydrology: rivers, lakes, basins.

Geometry pipeline

Icosahedron generation

Create 12 canonical vertices.

Create 20 triangular faces.

Normalize vertices to radius R.

Subdivision

For each triangle, compute midpoints.

Replace triangle with 4 smaller triangles.

Repeat N times.

Spherical projection

Normalize each vertex:

v = R * normalize(v)

Relaxation (Lloyd)

Move each vertex toward centroid of neighbors.

Reproject to sphere.

Repeat 5–10 iterations.

Note: Lloyd relaxation is simple but often requires many iterations. For better performance and faster convergence, consider these approaches:

Use Centroidal Voronoi Tessellation (CVT) variants with optimization to reduce iteration count.

Apply Laplacian smoothing constrained to the sphere for faster smoothing.

Employ Newton or quasi-Newton optimization methods to directly minimize mesh energy.

Combine a few Lloyd iterations with advanced smoothing or optimization for best tradeoff.

While some iteration is usually needed, these methods significantly reduce the total iterations compared to naive Lloyd relaxation, improving performance without sacrificing mesh quality.

Best practice: Some iteration is generally unavoidable, but hybrid approaches that mix a small number of Lloyd iterations with more advanced smoothing or optimization techniques provide the best [...]

If you want to explore or implement specific optimization techniques tailored to your engine, consider starting with Laplacian smoothing constrained to the sphere or CVT optimization methods, whi[...]

Dual mesh → Hex sphere

Compute triangle centroids.

Connect centroids of adjacent triangles.

Build polygon cells (mostly hexagons, 12 pentagons).

Data model

Cell structure

struct Cell {
    Vector3 position;
    int neighbors[6];
    float elevation;
    float temperature;
    float precipitation;
    int biome;
    int plate_id;
    bool is_ocean;
    int flow_to;
    int flow_accum;
}

Plate structure

struct Plate {
    int id;
    Vector2 motion;
    bool is_continental;
}

Tectonic simulation

Plate seeding

Pick P random cells as seeds.

Flood —fill to assign plate_id.

Plate motion

Assign random 2D motion vector per plate.

Boundary classification

Compare motion vectors along shared edges.

Classify as convergent, divergent, or transform.

Base elevation

Continental plates → higher baseline.

Oceanic plates → lower baseline.

Add mountains near convergent boundaries.

Add rifts near divergent boundaries.

Elevation generation

Noise fields

Use FBM noise:

Large scale → continents.

Mid scale → mountains and mountain range shaping.

Small scale → hills and terrain detail.

Mountains

Mountains are primarily generated near convergent plate boundaries during tectonic simulation, forming the base elevation. This base is then refined by mid-scale noise layers to create realistic [...]

Additional techniques for mountain shaping include:

Ridge generation along plate boundaries to simulate mountain chains.

Erosion approximations to soften and shape peaks and valleys.

Performance-friendly noise modulation to add variation without heavy computation.

Mountains also influence climate and hydrology by creating rain shadows and snow caps, affecting precipitation and biome distribution.

Combine tectonics + noise

e = e_tect + w1*nL + w2*nM + w3*nS

Sea level

Normalize elevation.

Mark is_ocean.

Smoothing

1–2 lightweight smoothing passes.

Climate model

Temperature

T = T_equator - |lat| * deltaT - elevation * lapse_rate

Wind bands

Equatorial → east —west.

Subtropical → west —east.

Mid —latitude → mixed.

Polar → weak.

Precipitation

Moisture starts over oceans.

Moves along wind direction.

Rains when forced upward by terrain.

Creates rain shadows.

Biome assignment

Based on temperature, precipitation, elevation. Examples:

Tundra

Boreal forest

Temperate forest

Grassland

Desert

Savanna

Rainforest

Hydrology

Flow direction

Each land cell flows to lowest neighbor.

Flow accumulation

Count upstream cells.

Rivers

Cells with accumulation above threshold.

Lakes

Cells with no lower neighbor.

Mesh Integrity

Overview

Mesh integrity ensures that all vertices and edges in the hex-sphere planet mesh are properly connected, avoiding gaps or seams during rendering and simulation.

Common Issues

Subdivision mismatch: Adjacent faces subdivide differently, leaving unaligned edges.

Indexing errors: Floating-point precision causes nearly identical vertices to be treated as separate.

Projection rounding: Sphere projection introduces small coordinate differences that break connectivity.

LOD seams: Multi-scale LOD transitions can leave gaps if higher-detail meshes don’t snap to coarser ones.

Solutions

Vertex snapping: Round projected coordinates to a fixed tolerance (e.g., 1e-6) before deduplication.

Edge registry: Maintain a hash map of edges during subdivision to ensure shared vertices are reused.

Seam correction pass: After each LOD build, run a stitching pass to merge duplicate vertices along borders.

Debug visualization: Render wireframe edges with color coding to highlight unshared edges.

Implementation Notes

Deduplication should occur immediately after projection to the sphere.

Edge registry must be consistent across all subdivision levels.

Seam correction is mandatory when transitioning between LOD levels.

Testing

Enable wireframe mode to visually inspect connectivity.

Log vertex counts before and after deduplication to confirm merges.

Run automated checks to ensure no dangling edges remain.

This section complements the Elevation and Hydrology systems by ensuring the underlying mesh is watertight and reliable for biome generation, city placement, and terrain simulation.

Procedural Planet – Multi‑Scale LOD Architecture

This document extends the existing README with three new core concepts required for a full universe‑to‑ground procedural planet engine:

Planet‑Scale LOD (LOD0)

Regional LOD (LOD1–3)

Ground LOD (LOD4+)

It also includes the technical documentation required to implement these systems inside the existing Rust/C++ engine.

1. Planet‑Scale LOD (LOD0)

Planet‑scale LOD represents the entire planet at low resolution. It is the foundation for all higher‑resolution refinements.

Goals

Represent the whole planet with minimal geometry

Provide large‑scale features: continents, oceans, tectonic plates

Maintain stable topology using the hex‑sphere

Ensure O(n) generation cost

Geometry

Base mesh: subdivided icosahedron

Converted to hex/pent cells

~10k–100k cells

Data

Low‑frequency elevation

Plate ID

Moisture & temperature (coarse)

Biome (coarse)

Algorithms

Tectonic simulation (plate drift, collisions, ridges)

Low‑frequency elevation noise

Climate simulation (Hadley/Ferrel/Polar cells)

Output

A complete but low‑detail planet suitable for space‑view rendering.

2. Regional LOD (LOD1–3)

Regional LOD refines the planet only where the camera approaches. Each base hex subdivides into smaller hexes.

Goals

Add medium‑scale detail: mountains, valleys, rivers

Improve coastline fidelity

Maintain seamless transitions between LODs

Keep memory usage low

Subdivision

Each hex subdivides into a local hex grid:

LOD1: 7 cells

LOD2: 19 cells

LOD3: 37 cells

Data

Medium‑frequency elevation

Local moisture & temperature

River flow direction & accumulation

Local biome refinement

Algorithms

Multi‑octave noise refinement

Local erosion simulation

River generation using flow accumulation

Climate downscaling

Output

Smooth transition from global to regional detail

Suitable for orbit and atmospheric entry

3. Ground LOD (LOD4+)

Ground LOD is activated when the camera is very close to the surface.

Goals

Provide high‑resolution terrain for ground exploration

Support vegetation, rocks, cities

Maintain performance on mobile

Geometry

Convert regional hex patch → heightmap tile

128×128 or 256×256 resolution

Tessellated on GPU

Data

High‑frequency elevation

Micro‑detail noise (rocks, cliffs)

Material masks (grass, sand, rock)

Object spawn maps (trees, props)

Algorithms

Heightmap extraction from hex grid

Micro‑noise layering

GPU tessellation or mesh shaders

Procedural object placement

Output

Fully detailed ground‑level terrain

Smooth transition from orbit to surface

Technical Documentation

LOD System Architecture

The LOD system is camera‑driven and streamed asynchronously.

LOD Selection

Each frame:

Compute distance from camera to surface

Determine required LOD level

Request generation tasks from Rust core

Stream results to GPU buffers

LOD Rules

| 10,000 km

Rust Core – Data Model

The engine uses a Structure of Arrays (SoA) layout.

Planet Data

elevation: Vec<u16>

temperature: Vec<u8>

moisture: Vec<u8>

plate_id: Vec<u16>

biome: Vec<u8>

flow_dir: Vec<u8>

flow_accum: Vec<u16>

LOD Data

Each LOD level stores:

Subdivision index

Parent cell reference

Local elevation refinement

Local climate refinement

Rust Core – Generation Pipeline

LOD0 Generation

Build hex‑sphere

Run tectonics

Generate low‑frequency elevation

Compute global climate

LOD1–3 Generation

Triggered when camera approaches.

Subdivide parent hex

Generate medium‑frequency elevation

Run local erosion

Compute rivers

Downscale climate

LOD4+ Generation

Triggered near ground.

Extract heightmap tile

Apply micro‑noise

Generate material masks

Spawn objects

C++ Rendering Pipeline

Geometry

Static planet mesh for LOD0

Dynamic patch meshes for LOD1–3

Heightmap‑based terrain for LOD4+

GPU Data

SSBO/TBO for dynamic attributes

Persistent mapped buffers

Triple buffering to avoid sync stalls

Shaders

Vertex: hex‑sphere deformation

Tessellation: ground LOD refinement

Fragment: biome‑based material blending

Streaming System

Goals

Avoid generating unused regions

Keep memory usage low

Maintain smooth transitions

Process

Camera requests LOD

Rust generates data asynchronously

C++ uploads to GPU

Old patches are freed when far away

Performance and memory considerations

Use compact types (int16, uint8).

Avoid erosion and fluid simulation.

Store only essential fields.

Keep all algorithms O(n).

Full generation pipeline summary

Build icosahedron.

Subdivide.

Project to sphere.

Relax vertices.

Build hex sphere.

Generate tectonic plates.

Compute base elevation.

Add noise layers.

Normalize elevation + sea level.

Compute temperature.

Compute wind + precipitation.

Assign biomes.

Compute rivers + lakes.

End of README.
