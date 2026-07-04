# Climate, Biomes & Hydrology

Detailed systems for weather simulation, biome classification, and water flow with pseudocode, complexity analysis, and test strategies.

---

## Climate System

### 1.1 Temperature Calculation

**Input:** latitude, elevation, sea level  
**Output:** temperature[cell]  
**Complexity:** O(n) — single pass

**Algorithm:**

Compute temperature based on latitude gradient and lapse rate (altitude cooling):

```
T = T_equator - |latitude| × deltaT_per_degree - elevation × lapse_rate
```

**Parameters:**
- T_equator: 30°C (equatorial temperature)
- deltaT_per_degree: 0.5°C per degree latitude
- lapse_rate: 6.5°C per km elevation (standard atmospheric)

**Implementation pseudocode:**

```rust
fn compute_temperature(
    positions: &[Vec3],
    elevation: &[f32],
    sea_level: f32,
    sphere_radius: f32,
) -> Vec<u8> {
    let mut temperature = Vec::new();
    
    for (pos, &elev) in positions.iter().zip(elevation.iter()) {
        // Calculate latitude (degrees from equator)
        let latitude_rad = pos.y.asin();  // y component on unit sphere
        let latitude_deg = latitude_rad.to_degrees();
        
        // Base temperature at sea level
        let mut temp = 30.0 - latitude_deg.abs() * 0.5;
        
        // Altitude cooling (convert elevation to km)
        let elevation_km = elev * 10.0;  // Assume elevation range [0, 1] → [0, 10] km
        temp -= elevation_km * 6.5;
        
        // Clamp to realistic range
        temp = temp.clamp(-50.0, 60.0);
        
        // Quantize to u8 (offset by 50 to store negative values)
        let quantized = ((temp + 50.0).clamp(0.0, 100.0)) as u8;
        temperature.push(quantized);
    }
    
    temperature
}
```

**Dequantization (for display/logic):**

```rust
fn dequantize_temperature(quantized: u8) -> f32 {
    quantized as f32 - 50.0  // Range: [-50, 50]°C
}
```

---

### 1.2 Wind Bands (Hadley/Ferrel/Polar Cells)

**Input:** latitude  
**Output:** wind_vector[cell]  
**Complexity:** O(n) — simple lookup

**Algorithm:**

Implement banded global wind model based on latitude:

```
0–30° latitude (Hadley cells):
    → Trade winds: east-to-west (westward)
    
30–60° latitude (Ferrel cells):
    → Westerlies: west-to-east (eastward)
    
60–90° latitude (Polar cells):
    → Polar easterlies: weak east-to-west
```

**Implementation pseudocode:**

```rust
fn compute_wind_bands(positions: &[Vec3]) -> Vec<Vec2> {
    let mut wind = Vec::new();
    
    for pos in positions {
        let latitude_rad = pos.y.asin();
        let latitude_deg = latitude_rad.to_degrees().abs();
        
        let (direction, magnitude) = if latitude_deg < 30.0 {
            // Trade winds (equatorial)
            (Vec2(-1.0, 0.0), 1.0)  // Westward
        } else if latitude_deg < 60.0 {
            // Westerlies (mid-latitude)
            (Vec2(1.0, 0.0), 0.8)  // Eastward
        } else {
            // Polar easterlies
            (Vec2(-1.0, 0.0), 0.3)  // Weak, eastward
        };
        
        wind.push(direction * magnitude);
    }
    
    wind
}
```

---

### 1.3 Precipitation (Orographic Lifting)

**Input:** elevation[cell], wind[], is_ocean[cell], neighbors[]  
**Output:** precipitation[cell]  
**Complexity:** O(n) — single pass with neighbor lookup

**Algorithm:**

Moisture sourced from oceans, advected along wind, creates rain shadows on lee sides:

```
1. Initialize moisture: ocean cells = 1.0, land cells = 0.5
2. For each cell:
   a. Get upwind neighbor (opposite of wind direction)
   b. If upwind neighbor is higher (orographic lifting):
      → Create rain (add to precipitation)
      → Reduce moisture (precipitation_loss)
   c. If downwind from mountains (lee side):
      → Create rain shadow (reduce precipitation)
   d. Propagate remaining moisture downwind
```

**Implementation pseudocode:**

```rust
fn compute_precipitation(
    positions: &[Vec3],
    elevation: &[f32],
    wind: &[Vec2],
    is_ocean: &[bool],
    neighbors: &[Vec<u32>],
) -> Vec<u8> {
    let mut precipitation = vec![0.0; positions.len()];
    let mut moisture = vec![0.0; positions.len()];
    
    // Initialize moisture from oceans
    for (i, &ocean) in is_ocean.iter().enumerate() {
        moisture[i] = if ocean { 1.0 } else { 0.5 };
    }
    
    // Advect moisture along wind, create orographic rain
    for (cell_idx, &wind_vec) in wind.iter().enumerate() {
        if moisture[cell_idx] < 0.01 {
            continue;  // No moisture to advect
        }
        
        let current_elev = elevation[cell_idx];
        
        // Find upwind neighbor (most opposite to wind direction)
        let mut best_upwind = cell_idx;
        let mut best_dot = -2.0;
        
        for &neighbor_idx in neighbors[cell_idx].iter() {
            let neighbor_pos = positions[neighbor_idx as usize];
            let cell_pos = positions[cell_idx];
            let to_neighbor = (neighbor_pos - cell_pos).normalize();
            
            let dot = to_neighbor.dot(wind_vec);
            if dot < best_dot {
                best_dot = dot;
                best_upwind = neighbor_idx as usize;
            }
        }
        
        // Orographic precipitation
        let upwind_elev = elevation[best_upwind];
        if upwind_elev > current_elev {
            // Upwind terrain forces air upward → rain
            let lift = upwind_elev - current_elev;
            let rain_amount = moisture[cell_idx] * lift * 0.5;
            
            precipitation[cell_idx] += rain_amount;
            moisture[cell_idx] -= rain_amount * 0.3;
        } else if upwind_elev < current_elev {
            // Downwind (lee) side → rain shadow
            let drop = current_elev - upwind_elev;
            let shadow_factor = (drop * 0.3).min(1.0);
            
            precipitation[cell_idx] -= shadow_factor * 100.0;
            moisture[cell_idx] -= shadow_factor * 0.1;
        }
        
        // Advect remaining moisture downwind
        let mut best_downwind = cell_idx;
        let mut best_wind_dot = -2.0;
        
        for &neighbor_idx in neighbors[cell_idx].iter() {
            let neighbor_pos = positions[neighbor_idx as usize];
            let cell_pos = positions[cell_idx];
            let to_neighbor = (neighbor_pos - cell_pos).normalize();
            
            let dot = to_neighbor.dot(wind_vec);
            if dot > best_wind_dot {
                best_wind_dot = dot;
                best_downwind = neighbor_idx as usize;
            }
        }
        
        moisture[best_downwind] += moisture[cell_idx] * 0.5;
    }
    
    // Quantize to u8
    let max_precip = precipitation.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let precipitation_u8 = precipitation.iter()
        .map(|&p| {
            let normalized = if max_precip > 0.0 { p / max_precip } else { 0.0 };
            (normalized * 255.0).clamp(0.0, 255.0) as u8
        })
        .collect();
    
    precipitation_u8
}
```

---

## Biome System

### 2.1 Biome Lookup Table

**Input:** temperature, precipitation, elevation  
**Output:** biome[cell]  
**Complexity:** O(n) — single lookup per cell

**Algorithm:**

Use 2D decision matrix (temperature vs precipitation) with elevation overrides:

| Temperature | Low Precip | Med Precip | High Precip |
|-------------|-----------|-----------|------------|
| Cold (<0°C) | Tundra | Tundra | Boreal |
| Cool (0–15°C) | Desert | Grassland | Temperate |
| Warm (15–25°C) | Desert | Savanna | Rainforest |
| Hot (>25°C) | Desert | Savanna | Rainforest |

**Elevation overrides:**
- elevation > 3000m → Alpine (overrides all)
- is_ocean → Water (overrides all)

**Biome enum:**

```rust
#[repr(u8)]
pub enum Biome {
    Water = 0,
    Desert = 1,
    Grassland = 2,
    Savanna = 3,
    Temperate = 4,
    Boreal = 5,
    Rainforest = 6,
    Tundra = 7,
    Alpine = 8,
}
```

---

### 2.2 Classification Algorithm

**Implementation pseudocode:**

```rust
fn classify_biomes(
    temperature: &[u8],
    precipitation: &[u8],
    elevation: &[u16],
    is_ocean: &[bool],
) -> Vec<u8> {
    let mut biome = Vec::new();
    
    for (idx, (&temp_q, &precip_q)) in temperature.iter().zip(precipitation.iter()).enumerate() {
        // Dequantize
        let temp = temp_q as f32 - 50.0;  // Range: [-50, 50]°C
        let precip = (precip_q as f32 / 255.0) * 1000.0;  // Range: [0, 1000]mm
        let elev = (elevation[idx] as f32 / 65535.0) * 10.0;  // Range: [0, 10]km
        
        let biome_type = if is_ocean[idx] {
            Biome::Water as u8
        } else if elev > 3.0 {
            Biome::Alpine as u8
        } else if temp < 0.0 {
            if precip > 500.0 {
                Biome::Boreal as u8
            } else {
                Biome::Tundra as u8
            }
        } else if temp < 15.0 {
            if precip < 250.0 {
                Biome::Desert as u8
            } else if precip < 500.0 {
                Biome::Grassland as u8
            } else {
                Biome::Temperate as u8
            }
        } else if temp < 25.0 {
            if precip < 200.0 {
                Biome::Desert as u8
            } else if precip < 400.0 {
                Biome::Savanna as u8
            } else {
                Biome::Rainforest as u8
            }
        } else {
            // temp >= 25.0
            if precip < 200.0 {
                Biome::Desert as u8
            } else if precip < 400.0 {
                Biome::Savanna as u8
            } else {
                Biome::Rainforest as u8
            }
        };
        
        biome.push(biome_type);
    }
    
    biome
}
```

---

## Hydrology System

### 3.1 Flow Direction Computation

**Input:** elevation[cell], neighbors[]  
**Output:** flow_to[cell] (neighbor index or NO_FLOW)  
**Complexity:** O(n) — single pass

**Algorithm:**

Each cell flows to its lowest neighbor (deterministic tie-breaking):

```
for each land cell c:
    lowest_neighbor = argmin(elevation[neighbors[c]])
    if elevation[lowest_neighbor] < elevation[c]:
        flow_to[c] = index_of_lowest_neighbor
    else:
        flow_to[c] = NO_FLOW  (sink/basin)
```

**Implementation pseudocode:**

```rust
const NO_FLOW: u8 = 255;

fn compute_flow_direction(
    elevation: &[u16],
    is_ocean: &[bool],
    neighbors: &[Vec<u32>],
) -> Vec<u8> {
    let mut flow_to = Vec::new();
    
    for (cell_idx, &ocean) in is_ocean.iter().enumerate() {
        if ocean {
            flow_to.push(NO_FLOW);  // Ocean cells don't flow
            continue;
        }
        
        let current_elev = elevation[cell_idx];
        let mut lowest_neighbor = None;
        let mut lowest_elev = current_elev;
        let mut lowest_idx = u8::MAX;
        
        for (neighbor_pos, &neighbor_idx) in neighbors[cell_idx].iter().enumerate() {
            let neighbor_elev = elevation[neighbor_idx as usize];
            
            if neighbor_elev < lowest_elev {
                lowest_elev = neighbor_elev;
                lowest_neighbor = Some(neighbor_idx);
                lowest_idx = neighbor_pos as u8;
            } else if neighbor_elev == lowest_elev && lowest_neighbor.is_none() {
                // Tie-breaking: pick smallest index
                if neighbor_idx < lowest_neighbor.unwrap_or(u32::MAX) {
                    lowest_neighbor = Some(neighbor_idx);
                    lowest_idx = neighbor_pos as u8;
                }
            }
        }
        
        if let Some(_) = lowest_neighbor {
            flow_to.push(lowest_idx);
        } else {
            flow_to.push(NO_FLOW);  // Sink cell
        }
    }
    
    flow_to
}
```

---

### 3.2 Flow Accumulation

**Input:** flow_to[cell]  
**Output:** flow_accum[cell]  
**Complexity:** O(n) — single topological pass

**Algorithm:**

Count upstream cells by traversing in reverse topological order:

```
1. Build reverse map: upstream[c] = [c' where flow_to[c'] == c]
2. Topological sort: cells with no downstream get processed first
3. for each cell c in reverse order:
     flow_accum[c] = 1 + sum(flow_accum[upstream[c]])
```

**Implementation pseudocode:**

```rust
fn compute_flow_accumulation(flow_to: &[u8]) -> Vec<u16> {
    let cell_count = flow_to.len();
    
    // Build reverse map: who flows into me?
    let mut upstream: Vec<Vec<u32>> = vec![Vec::new(); cell_count];
    for (cell, &destination) in flow_to.iter().enumerate() {
        if destination != NO_FLOW {
            upstream[destination as usize].push(cell as u32);
        }
    }
    
    // Topological sort (DFS from sinks)
    let mut visited = vec![false; cell_count];
    let mut topo_order = Vec::new();
    
    fn dfs(cell: usize, upstream: &[Vec<u32>], visited: &mut [bool], order: &mut Vec<u32>) {
        visited[cell] = true;
        for &up_cell in upstream[cell].iter() {
            if !visited[up_cell as usize] {
                dfs(up_cell as usize, upstream, visited, order);
            }
        }
        order.push(cell as u32);
    }
    
    for cell in 0..cell_count {
        if !visited[cell] {
            dfs(cell, &upstream, &mut visited, &mut topo_order);
        }
    }
    
    // Compute accumulation in topological order
    let mut flow_accum = vec![1u16; cell_count];
    for &cell in topo_order.iter() {
        let cell_idx = cell as usize;
        for &up_cell in upstream[cell_idx].iter() {
            flow_accum[cell_idx] = flow_accum[cell_idx]
                .saturating_add(flow_accum[up_cell as usize]);
        }
    }
    
    flow_accum
}
```

---

### 3.3 River & Lake Detection

**Input:** flow_accum[cell], flow_to[cell], elevation[cell]  
**Output:** is_river[cell], is_lake[cell]  
**Complexity:** O(n) — single pass

**Algorithm:**

Rivers form above accumulation threshold; lakes form at sinks:

```rust
fn detect_rivers_and_lakes(
    flow_accum: &[u16],
    flow_to: &[u8],
    elevation: &[u16],
    is_ocean: &[bool],
) -> (Vec<bool>, Vec<bool>) {
    const RIVER_THRESHOLD: u16 = 50;  // Cells upstream threshold
    
    let mut is_river = Vec::new();
    let mut is_lake = Vec::new();
    
    for (idx, (&accum, &flow)) in flow_accum.iter().zip(flow_to.iter()).enumerate() {
        // River: high accumulation
        is_river.push(!is_ocean[idx] && accum > RIVER_THRESHOLD);
        
        // Lake: sink cell (no outflow)
        is_lake.push(!is_ocean[idx] && flow == NO_FLOW && accum > 10);
    }
    
    (is_river, is_lake)
}
```

---

### 3.4 Hydrology Test Cases

```rust
#[test]
fn test_flow_direction_no_cycles() {
    let elevation = vec![0.5, 0.3, 0.7, 0.2, 0.6];
    let is_ocean = vec![false, false, false, false, false];
    let neighbors = vec![
        vec![1, 2],      // 0 → neighbors [1, 2]
        vec![0, 3],      // 1 → neighbors [0, 3]
        vec![0, 4],      // 2 → neighbors [0, 4]
        vec![1, 4],      // 3 → neighbors [1, 4]
        vec![2, 3],      // 4 → neighbors [2, 3]
    ];
    
    let flow_to = compute_flow_direction(&elevation, &is_ocean, &neighbors);
    
    // Verify no cycles
    for start in 0..flow_to.len() {
        let mut current = start;
        let mut steps = 0;
        while flow_to[current] != NO_FLOW && steps < 100 {
            current = flow_to[current] as usize;
            steps += 1;
        }
        assert!(steps < 100, "Cycle detected in flow network");
    }
}

#[test]
fn test_flow_accumulation_monotonic() {
    let flow_to = vec![1, 2, 3, NO_FLOW, 0];  // 0→1→2→3, 4→0
    let flow_accum = compute_flow_accumulation(&flow_to);
    
    // Verify accumulation increases downstream
    assert!(flow_accum[3] > flow_accum[2]);
    assert!(flow_accum[2] > flow_accum[1]);
    assert!(flow_accum[1] > flow_accum[0]);
}

#[test]
fn test_all_cells_reach_sink() {
    let elevation = vec![0.5, 0.3, 0.7, 0.2, 0.6];
    let is_ocean = vec![false, false, false, false, false];
    let neighbors = vec![
        vec![1, 2],
        vec![0, 3],
        vec![0, 4],
        vec![1, 4],
        vec![2, 3],
    ];
    
    let flow_to = compute_flow_direction(&elevation, &is_ocean, &neighbors);
    
    // Every cell must reach either ocean or sink
    for start in 0..flow_to.len() {
        let mut current = start;
        let mut steps = 0;
        while flow_to[current] != NO_FLOW && steps < 100 {
            current = flow_to[current] as usize;
            steps += 1;
        }
        assert!(steps < 100);
    }
}
```

---

## Climate System Integration

### 4.1 Complete Pipeline

```rust
fn generate_climate_and_biomes(
    positions: &[Vec3],
    elevation: &[f32],
    is_ocean: &[bool],
    neighbors: &[Vec<u32>],
    sea_level: f32,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // Step 1: Temperature
    let temperature = compute_temperature(positions, elevation, sea_level, 1.0);
    
    // Step 2: Wind bands
    let wind = compute_wind_bands(positions);
    
    // Step 3: Precipitation
    let precipitation = compute_precipitation(
        positions,
        elevation,
        &wind,
        is_ocean,
        neighbors,
    );
    
    // Step 4: Biomes
    let biome = classify_biomes(&temperature, &precipitation, elevation, is_ocean);
    
    (temperature, precipitation, biome)
}
```

---

## Test Cases Summary

| Test | Purpose |
|------|---------|
| `test_flow_direction_no_cycles` | Verify DAG property (no circular flow) |
| `test_flow_accumulation_monotonic` | Verify accumulation increases downstream |
| `test_all_cells_reach_sink` | Verify every cell reaches ocean or sink |
| `test_temperature_range` | Verify temperatures within realistic bounds |
| `test_biome_coverage` | Verify all biome types appear |
| `test_rain_shadows` | Verify precipitation follows orographic patterns |

---

## Next Steps

- [Rendering Pipeline →](04-RENDERING-PIPELINE.md) GPU architecture and shaders
- [Performance & Debugging →](06-PERFORMANCE-AND-DEBUGGING.md) Profiling and optimization
