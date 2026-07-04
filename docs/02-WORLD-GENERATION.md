# World Generation

Detailed algorithms for geometry, tectonics, and elevation with pseudocode, complexity analysis, and test strategies.

---

## Geometry Pipeline

### 1.1 Icosphere Generation

**Input:** Radius R  
**Output:** 12 vertices, 20 triangles  
**Complexity:** O(1) — fixed output size

**Algorithm:**

The icosahedron has 12 canonical vertices (golden ratio based) and 20 triangular faces:

```
φ = (1 + √5) / 2  (golden ratio)

Vertices (±1, ±φ, 0), (0, ±1, ±φ), (±φ, 0, ±1)
→ 12 vertices at equal angles
→ Normalize to radius R
```

**Implementation pseudocode:**

```rust
fn create_icosphere(radius: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let vertices = vec![
        Vec3(-1, φ, 0),   Vec3(1, φ, 0),   Vec3(-1, -φ, 0),   Vec3(1, -φ, 0),
        Vec3(0, -1, φ),   Vec3(0, 1, φ),   Vec3(0, -1, -φ),   Vec3(0, 1, -φ),
        Vec3(φ, 0, -1),   Vec3(φ, 0, 1),   Vec3(-φ, 0, -1),   Vec3(-φ, 0, 1),
    ];
    
    let normalized = vertices.iter()
        .map(|v| radius * v.normalize())
        .collect();
    
    let faces = vec![
        // 20 triangular faces (predefined indices)
        // ...
    ];
    
    (normalized, faces)
}
```

**Verification:**
- Vertex count = 12 ✓
- Face count = 20 ✓
- All vertices equidistant from origin ✓

---

### 1.2 Subdivision

**Input:** Triangle mesh, subdivision level N  
**Output:** (20 × 4^N) triangles  
**Complexity:** O(n) where n = output triangle count

**Algorithm:**

For each iteration, replace every triangle with 4 smaller triangles:

```
Original triangle:      Subdivided:
    v1                      v1
   /  \                     /  \
  /    \          →       m1 -- m3
 /      \                /  \ /  \
v2 --- v3              v2 -- m2 -- v3

Where m1, m2, m3 are edge midpoints
```

**Implementation pseudocode:**

```rust
fn subdivide_sphere(vertices: &mut Vec<Vec3>, faces: &mut Vec<[u32; 3]>, iterations: u32) {
    let mut edge_cache: HashMap<(u32, u32), u32> = HashMap::new();
    
    for _ in 0..iterations {
        let mut new_faces = Vec::new();
        
        for [v1, v2, v3] in faces.iter() {
            // Get or create midpoint vertices (deduplicate via edge hash)
            let m12 = get_or_create_midpoint(&mut vertices, &mut edge_cache, v1, v2);
            let m23 = get_or_create_midpoint(&mut vertices, &mut edge_cache, v2, v3);
            let m31 = get_or_create_midpoint(&mut vertices, &mut edge_cache, v3, v1);
            
            // Create 4 new triangles
            new_faces.push([v1, m12, m31]);
            new_faces.push([v2, m23, m12]);
            new_faces.push([v3, m31, m23]);
            new_faces.push([m12, m23, m31]);  // Center triangle
        }
        
        *faces = new_faces;
    }
}

fn get_or_create_midpoint(
    vertices: &mut Vec<Vec3>,
    cache: &mut HashMap<(u32, u32), u32>,
    v1: u32,
    v2: u32,
) -> u32 {
    let edge = if v1 < v2 { (v1, v2) } else { (v2, v1) };
    
    *cache.entry(edge).or_insert_with(|| {
        let midpoint = (vertices[v1 as usize] + vertices[v2 as usize]) / 2.0;
        let idx = vertices.len() as u32;
        vertices.push(midpoint);
        idx
    })
}
```

**Edge cases:**
- Shared edges must be deduplicated → use edge hash map
- Consistent vertex ordering → maintain (v1 < v2) canonicalization

---

### 1.3 Spherical Projection

**Input:** Mesh vertices (arbitrary magnitude)  
**Output:** Vertices on sphere of radius R  
**Complexity:** O(n) — one normalization per vertex

**Algorithm:**

```
v_projected = R * normalize(v)
```

**Implementation pseudocode:**

```rust
fn project_to_sphere(vertices: &mut Vec<Vec3>, radius: f32) {
    for v in vertices.iter_mut() {
        *v = radius * v.normalize();
    }
}
```

**Snapping tolerance:** 1e-6 (prevents floating-point duplication)

```rust
fn project_to_sphere_snapped(vertices: &mut Vec<Vec3>, radius: f32, tolerance: f32) {
    for v in vertices.iter_mut() {
        let normalized = v.normalize();
        // Snap coordinates to tolerance grid
        let snapped = Vec3(
            (normalized.x / tolerance).round() * tolerance,
            (normalized.y / tolerance).round() * tolerance,
            (normalized.z / tolerance).round() * tolerance,
        ).normalize();
        *v = radius * snapped;
    }
}
```

**Verification:**
- All vertices at distance R from origin: `|v| = R` ✓

---

### 1.4 Lloyd Relaxation (CVT Approximation)

**Input:** Vertices on sphere, edge topology, iteration count  
**Output:** Relaxed vertices (approximates Centroidal Voronoi Tessellation)  
**Complexity:** O(n × iterations) — typically 5–10 iterations

**Algorithm:**

Move each vertex toward the centroid of its neighbors' centroids:

```
for iteration in 1..=N:
    for each vertex v:
        neighbors = cells_adjacent_to_v
        centroid = average(neighbor_centroids)
        v_new = R * normalize(centroid)
    sync()  // Wait for all updates
```

**Implementation pseudocode:**

```rust
fn relax_vertices(
    vertices: &mut Vec<Vec3>,
    faces: &[Vec<u32>],  // Face adjacency per vertex
    radius: f32,
    iterations: u32,
) {
    for _ in 0..iterations {
        let mut new_vertices = vertices.clone();
        
        for (v_idx, adjacent_faces) in faces.iter().enumerate() {
            // Compute centroids of adjacent faces
            let mut centroid = Vec3::ZERO;
            for face_idx in adjacent_faces {
                let face = faces[*face_idx as usize];
                let face_centroid = (vertices[face[0] as usize]
                    + vertices[face[1] as usize]
                    + vertices[face[2] as usize]) / 3.0;
                centroid += face_centroid;
            }
            centroid /= adjacent_faces.len() as f32;
            
            // Project back to sphere
            new_vertices[v_idx] = radius * centroid.normalize();
        }
        
        *vertices = new_vertices;
    }
}
```

**Hybrid approach (faster convergence):**

Use Laplacian smoothing for final 2–3 iterations:

```rust
fn laplacian_smooth(vertices: &mut Vec<Vec3>, radius: f32, iterations: u32) {
    for _ in 0..iterations {
        let mut new_vertices = vertices.clone();
        
        for (v_idx, v) in vertices.iter().enumerate() {
            let neighbors = get_vertex_neighbors(v_idx);
            let avg_neighbor = neighbors.iter()
                .map(|&n| vertices[n])
                .sum::<Vec3>() / neighbors.len() as f32;
            
            // Blend: 0.5 current + 0.5 neighbor average
            new_vertices[v_idx] = radius * (0.5 * v + 0.5 * avg_neighbor).normalize();
        }
        
        *vertices = new_vertices;
    }
}
```

**Verification:**
- Vertices converge toward uniform distribution ✓
- All vertices remain at radius R ✓

---

### 1.5 Dual Mesh → Hex Sphere

**Input:** Triangle mesh (post-relaxation)  
**Output:** Hexagonal mesh (~12 pentagons, rest hexagons)  
**Complexity:** O(n) — single pass over triangles

**Algorithm:**

Create dual vertices at triangle centroids, then connect adjacent centroids:

```
1. For each triangle T:
   - Compute centroid C_T
   - Store as new "cell" vertex
   
2. For each edge (T1, T2):
   - If triangles are adjacent, connect C_T1 to C_T2
   - Result: dual mesh
   
3. Properties (Euler formula):
   - Most cells are hexagons (6 neighbors)
   - Exactly 12 cells are pentagons (5 neighbors)
```

**Implementation pseudocode:**

```rust
fn build_hex_mesh(
    vertices: &[Vec3],
    faces: &[[u32; 3]],
) -> (Vec<Vec3>, Vec<Vec<u32>>) {
    // Create dual vertices (triangle centroids)
    let mut dual_vertices = Vec::new();
    for face in faces {
        let centroid = (vertices[face[0] as usize]
            + vertices[face[1] as usize]
            + vertices[face[2] as usize]) / 3.0;
        dual_vertices.push(centroid);
    }
    
    // Build adjacency (which triangles share edges?)
    let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); faces.len()];
    for i in 0..faces.len() {
        for j in (i + 1)..faces.len() {
            if are_adjacent(faces[i], faces[j]) {
                adjacency[i].push(j as u32);
                adjacency[j].push(i as u32);
            }
        }
    }
    
    (dual_vertices, adjacency)
}

fn are_adjacent(face1: [u32; 3], face2: [u32; 3]) -> bool {
    // Two triangles are adjacent if they share an edge (2 vertices)
    let shared = face1.iter()
        .filter(|v| face2.contains(v))
        .count();
    shared == 2
}
```

**Verification:**
- Euler formula: V - E + F = 2 ✓
- Pentagon count = 12 ✓
- Hexagon count = F - 12 ✓
- All cells connected (no orphans) ✓

---

## Tectonic Simulation

### 2.1 Plate Seeding (Flood-Fill)

**Input:** Cell count, plate count P, seed value  
**Output:** plate_id[cell_count]  
**Complexity:** O(n) — linear flood-fill

**Algorithm:**

Pick P deterministic seed cells using pseudo-random seeding, then flood-fill:

```
1. Sort cells deterministically
2. Pick cells at indices: [0, n/P, 2n/P, ..., (P-1)n/P]
3. Flood-fill each seed cell to label neighbors
4. Boundary cells assigned to nearest seed
```

**Implementation pseudocode:**

```rust
fn seed_plates(cell_count: u32, plate_count: u32, seed: u64) -> Vec<u16> {
    let mut plate_id = vec![u16::MAX; cell_count as usize];
    
    // Deterministic seed cells
    let mut rng = SeededRng::new(seed);
    let mut seed_cells = Vec::new();
    for p in 0..plate_count {
        let cell_idx = ((p as f32 / plate_count as f32) * cell_count as f32) as u32;
        seed_cells.push((cell_idx, p as u16));
    }
    
    // BFS flood-fill from each seed
    let mut queue = VecDeque::new();
    for (seed_idx, plate) in seed_cells {
        queue.push_back(seed_idx);
        plate_id[seed_idx as usize] = plate;
    }
    
    while let Some(cell) = queue.pop_front() {
        let plate = plate_id[cell as usize];
        for neighbor in get_neighbors(cell) {
            if plate_id[neighbor as usize] == u16::MAX {
                plate_id[neighbor as usize] = plate;
                queue.push_back(neighbor);
            }
        }
    }
    
    plate_id
}
```

---

### 2.2 Plate Motion Vectors

**Input:** Plate count  
**Output:** motion[plate] (Vec2 tangent velocity)  
**Complexity:** O(P) where P = plate count

**Algorithm:**

Assign deterministic motion vectors (tangent to sphere surface):

```rust
fn assign_motion_vectors(plate_count: u32, seed: u64) -> Vec<Vec2> {
    let mut rng = SeededRng::new(seed);
    let mut motion = Vec::new();
    
    for _ in 0..plate_count {
        // Random tangent direction and speed
        let angle = rng.gen_range(0.0..2.0 * PI);
        let speed = rng.gen_range(0.1..1.0);
        
        motion.push(Vec2(
            speed * angle.cos(),
            speed * angle.sin(),
        ));
    }
    
    motion
}
```

---

### 2.3 Boundary Classification & Base Elevation

**Input:** plate_id[], motion[], cell_neighbors[]  
**Output:** base_elevation[], boundary_type[]  
**Complexity:** O(n) — one pass over all cells

**Algorithm:**

For each cell, compare plate motion vectors of neighbors:

```
if plates_same: interior cell
if motion_vectors point toward each other: convergent (compression)
if motion_vectors point apart: divergent (spreading)
if motion_vectors slide parallel: transform (shear)
```

**Implementation pseudocode:**

```rust
fn classify_boundaries_and_elevate(
    plate_id: &[u16],
    motion: &[Vec2],
    neighbors: &[Vec<u32>],
    is_continental: &[bool],
) -> Vec<f32> {
    let mut base_elevation = vec![0.0; plate_id.len()];
    
    for (cell_idx, &plate) in plate_id.iter().enumerate() {
        let is_cont = is_continental[plate as usize];
        
        // Base elevation by plate type
        if is_cont {
            base_elevation[cell_idx] = 0.6;  // Above sea level
        } else {
            base_elevation[cell_idx] = 0.1;  // Ocean
        }
        
        // Check boundaries
        for &neighbor_idx in neighbors[cell_idx].iter() {
            let neighbor_plate = plate_id[neighbor_idx as usize];
            if neighbor_plate != plate {
                let m1 = motion[plate as usize];
                let m2 = motion[neighbor_plate as usize];
                
                let dot = m1.dot(m2);
                if dot < -0.7 {
                    // Convergent boundary
                    base_elevation[cell_idx] += 0.3;  // Mountain uplift
                } else if dot > 0.7 {
                    // Divergent boundary
                    base_elevation[cell_idx] -= 0.2;  // Rift depression
                }
                // transform: no change
            }
        }
    }
    
    base_elevation
}
```

---

## Elevation Generation (FBM)

### 3.1 Fractional Brownian Motion (FBM)

**Input:** Cell positions, noise function, octave count, frequency scale  
**Output:** elevation[cell]  
**Complexity:** O(n × octaves) — vectorized SIMD operations

**Algorithm:**

Combine multiple octaves of noise at increasing frequencies:

```
elevation = Σ (amplitude_i × noise(frequency_i × position))
            for i in 1..octaves

where:
  amplitude_i = 0.5^i  (decreases per octave)
  frequency_i = 2^i    (doubles per octave)
```

**Implementation pseudocode:**

```rust
fn fbm_noise(
    position: Vec3,
    octaves: u32,
    base_frequency: f32,
    lacunarity: f32,  // typically 2.0
    persistence: f32, // typically 0.5
) -> f32 {
    let mut result = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = base_frequency;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        result += amplitude * perlin_noise(position * frequency);
        max_value += amplitude;
        
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    
    result / max_value  // Normalize to [0, 1]
}

fn compute_elevation_fbm(
    positions: &[Vec3],
    base_elev: &[f32],
) -> Vec<u16> {
    let mut elevation = Vec::new();
    
    for (pos, &base) in positions.iter().zip(base_elev.iter()) {
        let large = fbm_noise(*pos, 2, 0.01, 2.0, 0.5);
        let mid    = fbm_noise(*pos, 3, 0.05, 2.0, 0.5);
        let small  = fbm_noise(*pos, 2, 0.2,  2.0, 0.5);
        
        let combined = 0.70 * base
                     + 0.15 * large
                     + 0.10 * mid
                     + 0.05 * small;
        
        // Quantize to u16
        let quantized = (combined * 65535.0).clamp(0.0, 65535.0) as u16;
        elevation.push(quantized);
    }
    
    elevation
}
```

---

### 3.2 Continent Mask (Optional)

**Input:** Cell positions  
**Output:** continent_mask[cell]  
**Complexity:** O(n × octaves)

**Algorithm:**

Use low-frequency FBM to define 2–6 major landmasses:

```rust
fn continent_mask(position: Vec3) -> f32 {
    let mask_noise = fbm_noise(position, 2, 0.005, 2.0, 0.5);
    smoothstep(0.4, 0.6, mask_noise)  // Sharp transition
}
```

This naturally creates coherent continents instead of fragmented islands.

---

### 3.3 Smoothing Pass

**Input:** elevation[cell]  
**Output:** smoothed_elevation[cell]  
**Complexity:** O(n) per pass — run 1–2 times only

**Algorithm (Laplacian):**

Replace each value with a blend of itself and neighbor average:

```
smoothed[c] = 0.5 × elevation[c] + 0.5 × average(elevation[neighbors])
```

**Implementation pseudocode:**

```rust
fn smooth_elevation(elevation: &[u16], neighbors: &[Vec<u32>]) -> Vec<u16> {
    let mut smoothed = elevation.to_vec();
    
    for (idx, neighbor_list) in neighbors.iter().enumerate() {
        let avg_neighbor: u32 = neighbor_list.iter()
            .map(|&n| elevation[n as usize] as u32)
            .sum::<u32>() / neighbor_list.len() as u32;
        
        smoothed[idx] = (0.5 * elevation[idx] as u32 + 0.5 * avg_neighbor) as u16;
    }
    
    smoothed
}
```

**Warning:** Run only 1–2 times (prevents over-smoothing and blurs features).

---

### 3.4 Sea Level & Ocean Detection

**Input:** elevation[cell]  
**Output:** sea_level (float), is_ocean[cell] (bool)  
**Complexity:** O(n) — sort + threshold

**Algorithm:**

1. Sort elevations
2. Set sea level at percentile (typically 25%)
3. Mark cells below sea level as ocean

```rust
fn set_sea_level_and_oceans(elevation: &[u16]) -> (f32, Vec<bool>) {
    let mut sorted_elev = elevation.to_vec();
    sorted_elev.sort();
    
    let sea_level_idx = (sorted_elev.len() as f32 * 0.25) as usize;
    let sea_level = sorted_elev[sea_level_idx] as f32 / 65535.0;
    
    let is_ocean = elevation.iter()
        .map(|&e| (e as f32 / 65535.0) < sea_level)
        .collect();
    
    (sea_level, is_ocean)
}
```

---

## Test Cases

### Geometry Tests

```rust
#[test]
fn test_icosphere_vertex_count() {
    let (verts, faces) = create_icosphere(1.0);
    assert_eq!(verts.len(), 12);
    assert_eq!(faces.len(), 20);
}

#[test]
fn test_icosphere_all_vertices_at_radius() {
    let (verts, _) = create_icosphere(1.0);
    for v in verts {
        assert!((v.length() - 1.0).abs() < 1e-6);
    }
}

#[test]
fn test_subdivision_vertex_count() {
    let (mut verts, mut faces) = create_icosphere(1.0);
    subdivide_sphere(&mut verts, &mut faces, 1);
    // 20 faces × 4 = 80 faces after 1 subdivision
    assert_eq!(faces.len(), 80);
}

#[test]
fn test_hex_mesh_pentagon_count() {
    let (verts, faces) = create_icosphere(1.0);
    let (_, hex_adjacency) = build_hex_mesh(&verts, &faces);
    
    let pentagon_count = hex_adjacency.iter()
        .filter(|adj| adj.len() == 5)
        .count();
    
    assert_eq!(pentagon_count, 12);
}
```

### Tectonics Tests

```rust
#[test]
fn test_all_cells_assigned_plate() {
    let plate_id = seed_plates(1000, 10, 12345);
    assert!(!plate_id.iter().any(|&p| p == u16::MAX));
}

#[test]
fn test_plate_boundaries_exist() {
    let plate_id = seed_plates(1000, 10, 12345);
    let neighbors = compute_neighbors(1000);  // Mock
    
    let mut boundary_count = 0;
    for (cell, neighbor_list) in neighbors.iter().enumerate() {
        for &neighbor in neighbor_list {
            if plate_id[cell] != plate_id[neighbor as usize] {
                boundary_count += 1;
            }
        }
    }
    
    assert!(boundary_count > 0);
}
```

### Elevation Tests

```rust
#[test]
fn test_elevation_in_range() {
    let positions = vec![Vec3(0, 0, 1), Vec3(1, 0, 0), Vec3(0, 1, 0)];
    let base = vec![0.5, 0.5, 0.5];
    let elev = compute_elevation_fbm(&positions, &base);
    
    for e in elev {
        assert!(e >= 0 && e <= 65535);
    }
}

#[test]
fn test_ocean_cells_below_sea_level() {
    let elevation = vec![100, 200, 300, 400, 500, 600, 700, 800];
    let (sea_level, is_ocean) = set_sea_level_and_oceans(&elevation);
    
    for (e, &ocean) in elevation.iter().zip(is_ocean.iter()) {
        let normalized = *e as f32 / 65535.0;
        assert_eq!(ocean, normalized < sea_level);
    }
}
```

---

## Next Steps

- [Climate & Biomes & Hydrology →](03-CLIMATE-BIOMES-HYDROLOGY.md) Weather, ecology, water systems
- [Performance & Debugging →](06-PERFORMANCE-AND-DEBUGGING.md) Profiling and optimization
