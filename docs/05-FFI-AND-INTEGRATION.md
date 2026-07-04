# FFI & Integration Layer

Explicit Rust ↔ C++ contract, memory ownership model, error handling, and safety guarantees.

---

## Memory Ownership Model

### Fundamental Rule

**Rust owns all data. C++ holds non-owning references only.**

```
Rust (owner)           C++ (borrower)
┌──────────────┐       ┌──────────────┐
│ Allocate     │       │              │
│ Generate     │───→   │ Read         │
│ Manage       │       │ Upload GPU   │
│ Deallocate   │       │ Never modify │
└──────────────┘       └──────────────┘
```

---

### Lifetime Guarantee

1. **Allocation:** Rust allocates SoA arrays during world generation
2. **Export:** Rust passes raw pointers to C++ via FFI
3. **Usage:** C++ may only read/upload to GPU (no modification)
4. **Modification:** Rust may only modify between LOD transitions (safe points)
5. **Deallocation:** Rust deallocates via `free_planet()` call

**Invariant:** All pointers remain valid until `free_planet()` is called.

---

## Rust Exports

### 1.1 Core Generation Function

```rust
#[no_mangle]
pub extern "C" fn generate_planet(
    seed: u64,
    cell_count: u32,
    plate_count: u32,
) -> *mut PlanetData {
    match PlanetData::generate(seed, cell_count, plate_count) {
        Ok(planet) => Box::into_raw(Box::new(planet)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn free_planet(ptr: *mut PlanetData) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}
```

---

### 1.2 Data Export Structure

```rust
#[repr(C)]
pub struct PlanetData {
    // SoA arrays (read-only for C++)
    pub elevation_ptr: *const u16,
    pub temperature_ptr: *const u8,
    pub moisture_ptr: *const u8,
    pub biome_ptr: *const u8,
    pub plate_id_ptr: *const u16,
    pub flow_dir_ptr: *const u8,
    pub flow_accum_ptr: *const u16,
    
    // Geometry
    pub vertex_positions_ptr: *const f32,  // vec3 per cell × 6 neighbors
    pub vertex_normals_ptr: *const f32,    // vec3 per vertex
    pub vertex_colors_ptr: *const u32,     // RGBA per vertex
    
    // Topology
    pub indices_ptr: *const u16,
    pub neighbors_ptr: *const u32,         // [6] per cell
    
    // Metadata
    pub cell_count: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub patch_count: u32,
    
    // Patch info (array of PatchMetadata)
    pub patches_ptr: *const PatchMetadata,
}

#[repr(C)]
pub struct PatchMetadata {
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub centroid: [f32; 3],
    pub bounds_radius: f32,
}
```

---

### 1.3 Error Handling Enum

```rust
#[repr(u32)]
pub enum GenerationError {
    Success = 0,
    InvalidSeed = 1,
    InvalidCellCount = 2,
    InvalidPlateCount = 3,
    MeshGenerationFailed = 4,
    TectonicsComputationFailed = 5,
    SerializationFailed = 6,
    OutOfMemory = 7,
}

#[no_mangle]
pub extern "C" fn generate_planet_with_error(
    seed: u64,
    cell_count: u32,
    plate_count: u32,
    out_error: *mut u32,
) -> *mut PlanetData {
    if out_error.is_null() {
        return std::ptr::null_mut();
    }
    
    match PlanetData::generate_safe(seed, cell_count, plate_count) {
        Ok(planet) => {
            unsafe { *out_error = GenerationError::Success as u32; }
            Box::into_raw(Box::new(planet))
        }
        Err(e) => {
            unsafe { *out_error = e as u32; }
            std::ptr::null_mut()
        }
    }
}
```

---

## C++ Integration Layer

### 2.1 RAII Wrapper Class

```cpp
class PlanetEngine {
private:
    PlanetData* planet_data_;
    uint32_t generation_error_;
    
    // Non-copyable
    PlanetEngine(const PlanetEngine&) = delete;
    PlanetEngine& operator=(const PlanetEngine&) = delete;
    
public:
    PlanetEngine(uint64_t seed, uint32_t cells, uint32_t plates)
        : planet_data_(nullptr), generation_error_(0) {
        planet_data_ = generate_planet_with_error(seed, cells, plates, &generation_error_);
    }
    
    ~PlanetEngine() {
        if (planet_data_ != nullptr) {
            free_planet(planet_data_);
            planet_data_ = nullptr;
        }
    }
    
    bool is_valid() const {
        return planet_data_ != nullptr && generation_error_ == 0;
    }
    
    const char* error_string() const {
        switch (generation_error_) {
            case 0: return "Success";
            case 1: return "Invalid seed";
            case 2: return "Invalid cell count";
            case 3: return "Invalid plate count";
            case 4: return "Mesh generation failed";
            case 5: return "Tectonics computation failed";
            case 6: return "Serialization failed";
            case 7: return "Out of memory";
            default: return "Unknown error";
        }
    }
    
    // Read-only accessors
    const uint16_t* get_elevations() const {
        if (!is_valid()) return nullptr;
        return planet_data_->elevation_ptr;
    }
    
    const uint8_t* get_biomes() const {
        if (!is_valid()) return nullptr;
        return planet_data_->biome_ptr;
    }
    
    const uint8_t* get_temperatures() const {
        if (!is_valid()) return nullptr;
        return planet_data_->temperature_ptr;
    }
    
    uint32_t cell_count() const {
        return planet_data_ ? planet_data_->cell_count : 0;
    }
    
    uint32_t vertex_count() const {
        return planet_data_ ? planet_data_->vertex_count : 0;
    }
    
    uint32_t index_count() const {
        return planet_data_ ? planet_data_->index_count : 0;
    }
    
    // Validate pointer before use
    bool validate_pointer(const void* ptr, size_t size) const {
        return ptr != nullptr && size > 0;
    }
};
```

---

### 2.2 GPU Uploader

```cpp
class GPUBufferManager {
private:
    vk::Device device_;
    vk::PhysicalDevice physical_device_;
    
public:
    GPUBufferManager(vk::Device device, vk::PhysicalDevice physical)
        : device_(device), physical_device_(physical) {}
    
    vk::Buffer upload_elevations(const PlanetEngine& engine) {
        if (!engine.is_valid()) {
            throw std::runtime_error("Invalid planet data");
        }
        
        const uint16_t* elevations = engine.get_elevations();
        uint32_t count = engine.cell_count();
        size_t size_bytes = count * sizeof(uint16_t);
        
        // Create GPU buffer
        vk::BufferCreateInfo create_info;
        create_info.size = size_bytes;
        create_info.usage = vk::BufferUsageFlagBits::eStorageBuffer |
                            vk::BufferUsageFlagBits::eTransferDst;
        create_info.sharingMode = vk::SharingMode::eExclusive;
        
        vk::Buffer buffer = device_.createBuffer(create_info);
        
        // Allocate memory
        vk::MemoryRequirements mem_req = device_.getBufferMemoryRequirements(buffer);
        vk::MemoryAllocateInfo alloc_info;
        alloc_info.allocationSize = mem_req.size;
        alloc_info.memoryTypeIndex = find_memory_type(mem_req.memoryTypeBits);
        
        vk::DeviceMemory memory = device_.allocateMemory(alloc_info);
        device_.bindBufferMemory(buffer, memory, 0);
        
        // Copy data (via staging buffer on mobile)
        vk::Buffer staging = create_staging_buffer(size_bytes, elevations);
        copy_buffer(staging, buffer, size_bytes);
        device_.destroyBuffer(staging);
        
        return buffer;
    }
    
private:
    uint32_t find_memory_type(uint32_t filter) const {
        // Implementation omitted for brevity
        return 0;
    }
    
    vk::Buffer create_staging_buffer(size_t size, const void* data) const {
        // Implementation omitted for brevity
        return vk::Buffer();
    }
    
    void copy_buffer(vk::Buffer src, vk::Buffer dst, size_t size) const {
        // Implementation omitted for brevity
    }
};
```

---

## Safety Guarantees

### 3.1 Rust Promises

- ✅ All pointers valid until `free_planet()` called
- ✅ All data initialized (no undefined values)
- ✅ Memory layout matches C expectations (`#[repr(C)]`)
- ✅ No null pointers in valid PlanetData
- ✅ Array sizes match metadata (cell_count, vertex_count, index_count)
- ✅ Single-threaded access (generation completes before export)

### 3.2 C++ Promises

- ✅ No dereferencing after `free_planet()`
- ✅ No writing to exported data (read-only)
- ✅ No alignment assumptions beyond `repr(C)`
- ✅ Validate pointers before use
- ✅ Check error codes before accessing data

---

## Thread Safety

### 3.3 Access Model

```
Frame N:
┌─────────────┬──────────────────┬──────────────┐
│ Render old  │ GPU upload new   │ Rust gen LOD │
│ (read)      │ (read)           │ (compute)    │
└─────────────┴──────────────────┴──────────────┘

Sync point (free_planet + generate_planet)

Frame N+1:
┌─────────────┬──────────────────┬──────────────┐
│ Render new  │ GPU upload new   │ Rust gen LOD │
│ (read)      │ (read)           │ (compute)    │
└─────────────┴──────────────────┴──────────────┘
```

**Rules:**
1. Only one active `PlanetData*` at a time
2. Rust may not modify until LOD transition begins
3. C++ may read until new `PlanetData*` becomes active
4. Sync via `free_planet()` before `generate_planet()`

---

## Error Handling Pattern

### 3.4 C++ Usage Example

```cpp
int main() {
    try {
        // Create planet
        PlanetEngine engine(12345, 100000, 12);
        
        if (!engine.is_valid()) {
            std::cerr << "Generation failed: " << engine.error_string() << std::endl;
            return 1;
        }
        
        // Upload to GPU
        GPUBufferManager gpu_manager(device, physical_device);
        vk::Buffer elev_buffer = gpu_manager.upload_elevations(engine);
        
        // Render
        render_frame(elev_buffer, engine.cell_count());
        
        // engine goes out of scope → ~PlanetEngine() calls free_planet()
        
    } catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        return 1;
    }
    
    return 0;
}
```

---

## Serialization Contract

### 3.5 Planet Snapshots (Optional)

```rust
#[no_mangle]
pub extern "C" fn serialize_planet(
    planet: *const PlanetData,
    out_buffer: *mut u8,
    buffer_size: u32,
    out_size: *mut u32,
) -> u32 {
    if planet.is_null() {
        return GenerationError::InvalidSeed as u32;
    }
    
    // Use bincode or MessagePack
    match bincode::serialize(&(*planet)) {
        Ok(bytes) => {
            if bytes.len() > buffer_size as usize {
                return GenerationError::SerializationFailed as u32;
            }
            
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buffer, bytes.len());
                *out_size = bytes.len() as u32;
            }
            
            GenerationError::Success as u32
        }
        Err(_) => GenerationError::SerializationFailed as u32,
    }
}

#[no_mangle]
pub extern "C" fn deserialize_planet(
    buffer: *const u8,
    buffer_size: u32,
) -> *mut PlanetData {
    if buffer.is_null() {
        return std::ptr::null_mut();
    }
    
    let slice = unsafe { std::slice::from_raw_parts(buffer, buffer_size as usize) };
    
    match bincode::deserialize::<PlanetData>(slice) {
        Ok(planet) => Box::into_raw(Box::new(planet)),
        Err(_) => std::ptr::null_mut(),
    }
}
```

---

## Integration Checklist

- [ ] FFI functions declared and exported
- [ ] Error enum defined and tested
- [ ] C++ wrapper class created (RAII)
- [ ] Pointer validation implemented
- [ ] GPU upload tested
- [ ] Memory ownership verified
- [ ] Thread safety model understood
- [ ] Error handling tested
- [ ] Serialization/deserialization (if needed)
- [ ] Documentation updated with FFI contract

---

## Common Pitfalls

| Issue | Solution |
|-------|----------|
| Use-after-free | Validate pointer before access; check error codes |
| Buffer overflow | Verify array sizes match metadata |
| Alignment issues | Use `#[repr(C)]` for all exported structs |
| Dangling pointers | Call `free_planet()` before new `generate_planet()` |
| Data races | Single-threaded at sync points; LOD transitions block |
| Serialization version mismatch | Version field in serialized header |

---

## Next Steps

- [Performance & Debugging →](06-PERFORMANCE-AND-DEBUGGING.md) Profiling and optimization
- [Rendering Pipeline →](04-RENDERING-PIPELINE.md) GPU integration details
