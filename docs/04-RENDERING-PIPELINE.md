# Rendering Pipeline

GPU architecture, mesh batching, shader implementation, and LOD streaming for Vulkan (Android) and Metal (iOS).

---

## GPU Architecture

### 1.1 Patch-Based Rendering System

**Goal:** Minimize draw calls while maintaining efficient culling and LOD management.

**Strategy:**
- Divide sphere into **20–60 patches**
- Each patch contains **1–4k hexagonal cells**
- One draw call per patch (if visible)
- Frustum culling at patch level

**Patch sizing trade-offs:**

| Patches | Cells/Patch | Draw Calls | GPU Memory | Culling Efficiency |
|---------|------------|-----------|------------|-------------------|
| 20 | 5–10k | 20 | High | Low (coarse) |
| 60 | 1–4k | 60 | Low | High (fine) |

**Recommendation:** Start with 60 patches for optimal balance.

---

### 1.2 Vertex Format & Memory Layout

**Interleaved vertex structure (28 bytes per vertex):**

```c
struct Vertex {
    half normal[3];              // 6 bytes (half-precision)
    float position[3];           // 12 bytes
    uint8_t biome;               // 1 byte
    uint8_t padding;             // 1 byte (alignment)
    uint16_t elevation_offset;   // 2 bytes (LOD support)
    uint8_t color[4];            // 4 bytes (SRGB)
};
// Total: 28 bytes (cache-aligned)
```

**Why interleaved?**
- ✅ Better cache locality (all data for one vertex in ~28 bytes)
- ✅ GPU prefetching more efficient
- ✅ Reduced bandwidth on mobile

**Why half-precision normals?**
- ✅ Saves 6 bytes per vertex
- ✅ Sufficient quality for normal mapping
- ✅ Shader auto-converts to f32

---

### 1.3 GPU Memory Layout

**Static buffers (uploaded once):**

```rust
// Vertex buffer: one vertex per hex cell corner
let vertex_buffer_size = vertex_count * 28;  // bytes

// Index buffer: 6 triangles per hexagon (3 indices each)
let index_buffer_size = index_count * 2;  // u16 indices

// Patch metadata: offsets and counts
let patch_metadata: Vec<PatchInfo> = vec![
    PatchInfo { vertex_offset: 0, index_offset: 0, index_count: 1024 },
    PatchInfo { vertex_offset: 512, index_offset: 1024, index_count: 2048 },
    // ...
];
```

**Dynamic buffers (updated per LOD transition):**

```rust
// SSBO: elevation offsets for current LOD
layout(std430, binding = 1) buffer ElevationSSBO {
    float elevation_offset[];
};

// TBO: biome indices (can reuse if LOD doesn't change biomes)
layout(binding = 2) uniform samplerBuffer BiomeTBO;

// TBO: temperature & moisture (for visual feedback)
layout(binding = 3) uniform samplerBuffer ClimateTBO;
```

**Update strategy:**

```cpp
void update_gpu_buffers(const PlanetEngine& engine, LODLevel lod) {
    const uint16_t* elevations = engine.get_elevations();
    uint32_t count = engine.cell_count();
    
    // Persistent-mapped buffer for zero-copy update
    float* elevation_data = (float*)elevation_ssbo.persistent_map;
    
    for (uint32_t i = 0; i < count; ++i) {
        elevation_data[i] = (elevations[i] / 65535.0f) * elevation_scale;
    }
    
    elevation_ssbo.mark_updated();
}
```

---

### 1.4 Command Buffer Strategy

#### Vulkan Path (Android)

```cpp
void record_command_buffer_vulkan(vk::CommandBuffer cmd_buf) {
    cmd_buf.beginRenderPass(render_pass_info, vk::SubpassContents::eInline);
    
    // Bind static buffers (once per frame)
    cmd_buf.bindVertexBuffers(0, {vertex_buffer}, {0});
    cmd_buf.bindIndexBuffer(index_buffer, 0, vk::IndexType::eUint16);
    
    // Bind dynamic buffers
    cmd_buf.bindDescriptorSets(
        vk::PipelineBindPoint::eGraphics,
        pipeline_layout,
        0,
        {descriptor_set},
        {}
    );
    
    // One draw call per visible patch
    for (const auto& patch : visible_patches) {
        cmd_buf.drawIndexed(
            patch.index_count,
            1,
            patch.index_offset,
            patch.vertex_offset,
            0
        );
    }
    
    cmd_buf.endRenderPass();
}
```

#### Metal Path (iOS)

```objc
void record_command_buffer_metal(id<MTLCommandBuffer> cmd_buf) {
    id<MTLRenderCommandEncoder> render_encoder = 
        [cmd_buf renderCommandEncoderWithDescriptor:render_pass_descriptor];
    
    [render_encoder setRenderPipelineState:pipeline_state];
    
    // Bind buffers
    [render_encoder setVertexBuffer:vertex_buffer offset:0 atIndex:0];
    [render_encoder setVertexBuffer:elevation_ssbo offset:0 atIndex:1];
    
    // Draw patches
    for (const auto& patch : visible_patches) {
        [render_encoder drawIndexedPrimitives:MTLPrimitiveTypeTriangle
                                  indexCount:patch.index_count
                                   indexType:MTLIndexTypeUInt16
                                 indexBuffer:index_buffer
                           indexBufferOffset:patch.index_offset];
    }
    
    [render_encoder endEncoding];
}
```

---

## Shader Implementation

### 2.1 Vertex Shader (Vulkan/GLSL)

```glsl
#version 450

// Input vertex attributes (interleaved)
layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in uint in_biome;
layout(location = 3) in uint in_elevation_offset;
layout(location = 4) in vec4 in_color;

// Uniform buffers
layout(std140, binding = 0) uniform Camera {
    mat4 view;
    mat4 projection;
    vec3 eye_pos;
    float time;
};

// Dynamic SSBO: elevation offsets
layout(std430, binding = 1) readonly buffer ElevationSSBO {
    float elevation_offsets[];
};

// Output to fragment shader
layout(location = 0) out VS_OUT {
    vec3 normal;
    vec3 world_pos;
    flat uint biome;
    vec4 color;
} vs_out;

void main() {
    // Apply elevation offset (for LOD blending)
    float elev_delta = elevation_offsets[gl_VertexIndex];
    vec3 displaced_pos = in_position + in_normal * elev_delta;
    
    // Transform to clip space
    gl_Position = projection * view * vec4(displaced_pos, 1.0);
    
    // Pass data to fragment shader
    vs_out.normal = normalize(mat3(view) * in_normal);
    vs_out.world_pos = (view * vec4(displaced_pos, 1.0)).xyz;
    vs_out.biome = in_biome;
    vs_out.color = in_color;
}
```

---

### 2.2 Fragment Shader

```glsl
#version 450

layout(location = 0) in VS_OUT {
    vec3 normal;
    vec3 world_pos;
    flat uint biome;
    vec4 color;
} fs_in;

// Biome palette lookup (256×1 texture)
layout(binding = 2) uniform sampler2D biome_palette;

// Optional: detail normal map
layout(binding = 3) uniform sampler2D detail_normal;

// Climate data (temperature/moisture visualization)
layout(binding = 4) uniform samplerBuffer climate_tbo;

layout(location = 0) out vec4 out_color;

void main() {
    // Sample biome color from palette
    vec4 biome_color = texelFetch(biome_palette, ivec2(fs_in.biome, 0), 0);
    
    // Phong lighting
    vec3 light_dir = normalize(vec3(1.0, 1.0, 1.0));
    vec3 view_dir = normalize(-fs_in.world_pos);
    
    float diff = max(dot(fs_in.normal, light_dir), 0.2);
    float spec = pow(max(dot(reflect(-light_dir, fs_in.normal), view_dir), 0.0), 32.0);
    
    vec3 result = biome_color.rgb * diff + vec3(1.0) * spec * 0.5;
    
    out_color = vec4(result, 1.0);
}
```

---

### 2.3 Biome Palette (Color Lookup)

Store as 256×1 RGBA texture:

```rust
fn create_biome_palette() -> Vec<u32> {
    vec![
        0xFF1E90FF,  // [0] Water: deep blue
        0xFFF4A460,  // [1] Desert: sandy
        0xFF90EE90,  // [2] Grassland: light green
        0xFFDAA520,  // [3] Savanna: golden
        0xFF228B22,  // [4] Temperate: forest green
        0xFF006400,  // [5] Boreal: dark green
        0xFF00AA00,  // [6] Rainforest: bright green
        0xFFFFFFFF,  // [7] Tundra: white
        0xFF808080,  // [8] Alpine: gray
    ]
}
```

Upload as texture:

```cpp
vk::Image palette_texture = device.createImage(
    vk::ImageCreateInfo()
        .setImageType(vk::ImageType::e2D)
        .setFormat(vk::Format::eR8G8B8A8Srgb)
        .setExtent({256, 1, 1})
        .setMipLevels(1)
        .setArrayLayers(1)
        .setUsage(vk::ImageUsageFlagBits::eSampled)
);
```

---

## LOD Streaming

### 3.1 LOD Level Definition

```rust
pub enum LODLevel {
    Planet = 0,      // 10k–100k cells, whole planet
    Regional = 1,    // 100k cells, subdivided hex regions
    Ground = 2,      // Heightmap tiles, high detail
}

pub struct LODConfig {
    pub distance_planet: f32,    // > 2km → LOD0
    pub distance_regional: f32,  // 500m–2km → LOD1–3
    pub distance_ground: f32,    // < 500m → LOD4+
}
```

---

### 3.2 Camera-Driven LOD Transitions

```rust
fn update_visible_patches(
    camera_pos: Vec3,
    all_patches: &[Patch],
    camera_frustum: &Frustum,
    lod_config: &LODConfig,
) -> Vec<PatchRenderJob> {
    let mut render_jobs = Vec::new();
    
    for patch in all_patches {
        // Frustum culling
        if !camera_frustum.intersects(&patch.bounds) {
            continue;
        }
        
        let distance = (patch.center - camera_pos).length();
        let desired_lod = if distance > lod_config.distance_planet {
            LODLevel::Planet
        } else if distance > lod_config.distance_regional {
            LODLevel::Regional
        } else {
            LODLevel::Ground
        };
        
        // LOD transition
        if patch.current_lod != desired_lod {
            request_lod_transition(patch, desired_lod);
        }
        
        render_jobs.push(PatchRenderJob {
            patch_id: patch.id,
            lod: desired_lod,
            distance,
        });
    }
    
    // Sort by distance (render far-to-near for depth prepass)
    render_jobs.sort_by(|a, b| b.distance.partial_cmp(&a.distance).unwrap());
    
    render_jobs
}
```

---

### 3.3 Patch Loading/Unloading

```rust
fn manage_patch_lifecycle(
    patches: &mut Vec<Patch>,
    render_jobs: &[PatchRenderJob],
    unload_distance: f32,
) {
    let visible_ids: HashSet<_> = render_jobs.iter().map(|j| j.patch_id).collect();
    
    for patch in patches.iter_mut() {
        if visible_ids.contains(&patch.id) {
            // Keep in memory
            patch.last_visible_frame = current_frame;
        } else if patch.distance > unload_distance {
            // Mark for unload
            patch.scheduled_for_unload = true;
        }
    }
    
    // Asynchronously unload scheduled patches
    patches.retain(|p| !p.scheduled_for_unload);
}
```

---

## Memory & Performance

### 4.1 GPU Memory Budget

**Target: <50 MB total on mobile**

| Component | 10k cells | 100k cells |
|-----------|-----------|------------|
| Vertex buffer | 300 KB | 3 MB |
| Index buffer | 200 KB | 2 MB |
| Elevation SSBO | 40 KB | 400 KB |
| Biome palette | 1 KB | 1 KB |
| Textures (detail) | 2 MB | 2 MB |
| **Total** | ~2.5 MB | ~7.4 MB |

---

### 4.2 Performance Targets

| Metric | Target |
|--------|--------|
| Draw calls per frame | 20–60 |
| GPU utilization | 70–90% |
| Vertex throughput | >100M verts/sec |
| Texture bandwidth | <500 MB/sec |
| Frame time (60 FPS) | <16.7 ms |

---

## Debugging & Visualization

### 5.1 Debug Visualization Modes

```glsl
// Add to fragment shader for debugging

#ifdef DEBUG_FLOW
    // Visualize flow accumulation
    float flow_accum = texelFetch(climate_tbo, gl_PrimitiveID).r;
    out_color = vec4(flow_accum, flow_accum, 0.0, 1.0);
#endif

#ifdef DEBUG_ELEVATION
    // Visualize elevation as height color
    float elev = length(vs_in.normal) * 0.5;  // Mock
    out_color = vec4(elev, elev, elev, 1.0);
#endif

#ifdef DEBUG_TEMPERATURE
    // Visualize temperature: blue (cold) → red (hot)
    float temp = texelFetch(climate_tbo, gl_PrimitiveID + 1).r;
    out_color = vec4(temp, 0.0, 1.0 - temp, 1.0);
#endif
```

---

### 5.2 Wireframe Rendering

```cpp
void render_wireframe(vk::CommandBuffer cmd_buf, bool show_seams) {
    // Create separate pipeline with VK_POLYGON_MODE_LINE
    vk::PipelineRasterizationStateCreateInfo rasterization_info;
    rasterization_info.polygonMode = vk::PolygonMode::eLine;
    
    // Record commands with wireframe pipeline
    cmd_buf.bindPipeline(vk::PipelineBindPoint::eGraphics, wireframe_pipeline);
    
    // Render all patches (no frustum culling)
    for (const auto& patch : all_patches) {
        cmd_buf.drawIndexed(patch.index_count, 1, patch.index_offset, 0, 0);
    }
}
```

---

## Integration Checklist

- [ ] Vertex/index buffers created and uploaded
- [ ] SSBO for elevation offsets allocated
- [ ] Biome palette texture created
- [ ] Shaders compiled (GLSL → SPIR-V for Vulkan)
- [ ] Pipeline state object created
- [ ] Descriptor sets bound
- [ ] Frustum culling implemented
- [ ] LOD transitions tested
- [ ] Patch loading/unloading verified
- [ ] Frame time profiled

---

## Next Steps

- [FFI & Integration →](05-FFI-AND-INTEGRATION.md) Rust ↔ C++ memory contract
- [Performance & Debugging →](06-PERFORMANCE-AND-DEBUGGING.md) Profiling and optimization
