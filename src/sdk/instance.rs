#![allow(dead_code)]

use std::sync::Arc;

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::instance;

/// Maximum children to iterate before giving up.
const MAX_CHILDREN: usize = 1000;

/// Global cache: ClassDescriptor address -> class name string.
static CLASS_NAME_CACHE: once_cell::sync::Lazy<dashmap::DashMap<u64, String>> =
    once_cell::sync::Lazy::new(|| dashmap::DashMap::with_capacity(64));

pub struct Instance {
    pub address: u64,
    memory: Arc<Memory>,
}

impl Instance {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self { address, memory }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        is_valid_address(self.address)
    }

    pub fn get_name(&self) -> String {
        let name_ptr = self.memory.read::<u64>(self.address + instance::name());
        if is_valid_address(name_ptr) {
            self.memory.read_string(name_ptr)
        } else {
            String::new()
        }
    }

    /// Cached via CLASS_NAME_CACHE.
    pub fn get_class_name(&self) -> String {
        let descriptor = self.memory.read::<u64>(self.address + instance::class_descriptor());
        if descriptor == 0 || !is_valid_address(descriptor) {
            return String::new();
        }

        // Fast path: descriptor already cached
        if let Some(cached) = CLASS_NAME_CACHE.get(&descriptor) {
            return cached.value().clone();
        }

        // Slow path: read from memory and cache
        let class_name_ptr = self.memory.read::<u64>(descriptor + instance::class_name());
        let name = if class_name_ptr != 0 {
            self.memory.read_string(class_name_ptr)
        } else {
            String::new()
        };

        if !name.is_empty() {
            CLASS_NAME_CACHE.insert(descriptor, name.clone());
        }
        name
    }

    pub fn get_parent(&self) -> Option<Instance> {
        let parent_addr = self.memory.read::<u64>(self.address + instance::parent());
        if is_valid_address(parent_addr) {
            Some(Instance::new(parent_addr, Arc::clone(&self.memory)))
        } else {
            None
        }
    }

    pub fn get_children(&self) -> Vec<Instance> {
        let start = self.memory.read::<u64>(self.address + instance::children_start());

        if !is_valid_address(start) {
            return Vec::new();
        }

        let end = self.memory.read::<u64>(start + instance::children_end());
        let mut children = Vec::with_capacity(32);
        let mut current = self.memory.read::<u64>(start);
        let mut iterations = 0;

        while current != end && iterations < MAX_CHILDREN {
            let instance_addr = self.memory.read::<u64>(current);
            if is_valid_address(instance_addr) {
                children.push(Instance::new(instance_addr, Arc::clone(&self.memory)));
            }
            current += std::mem::size_of::<usize>() as u64;
            iterations += 1;
        }

        children
    }

    /// Walks children inline, returns early on match.
    pub fn find_first_child(&self, name: &str) -> Option<Instance> {
        let start = self.memory.read::<u64>(self.address + instance::children_start());
        if !is_valid_address(start) {
            return None;
        }
        let end = self.memory.read::<u64>(start + instance::children_end());
        let mut current = self.memory.read::<u64>(start);
        let mut iterations = 0;

        while current != end && iterations < MAX_CHILDREN {
            let instance_addr = self.memory.read::<u64>(current);
            if is_valid_address(instance_addr) {
                let child = Instance::new(instance_addr, Arc::clone(&self.memory));
                if child.get_name().eq_ignore_ascii_case(name) {
                    return Some(child);
                }
            }
            current += std::mem::size_of::<usize>() as u64;
            iterations += 1;
        }
        None
    }

    /// Walks children inline, returns early on class match.
    pub fn find_first_child_by_class(&self, class_name: &str) -> Option<Instance> {
        let start = self.memory.read::<u64>(self.address + instance::children_start());
        if !is_valid_address(start) {
            return None;
        }
        let end = self.memory.read::<u64>(start + instance::children_end());
        let mut current = self.memory.read::<u64>(start);
        let mut iterations = 0;

        while current != end && iterations < MAX_CHILDREN {
            let instance_addr = self.memory.read::<u64>(current);
            if is_valid_address(instance_addr) {
                let child = Instance::new(instance_addr, Arc::clone(&self.memory));
                if child.get_class_name().eq_ignore_ascii_case(class_name) {
                    return Some(child);
                }
            }
            current += std::mem::size_of::<usize>() as u64;
            iterations += 1;
        }
        None
    }

    pub fn find_children<F>(&self, predicate: F) -> Vec<Instance>
    where
        F: Fn(&Instance) -> bool,
    {
        self.get_children()
            .into_iter()
            .filter(|child| predicate(child))
            .collect()
    }

    pub fn memory(&self) -> &Arc<Memory> {
        &self.memory
    }
    
    #[inline]
    pub fn get_primitive_address(&self) -> u64 {
        use crate::core::offsets::base_part;
        self.memory.read::<u64>(self.address + base_part::primitive())
    }
    
    /// Writes to instance address, NOT primitive.
    #[inline]
    pub fn set_transparency(&self, transparency: f32) {
        use crate::core::offsets::base_part;
        self.memory.write(self.address + base_part::transparency(), transparency);
    }
    
    #[inline]
    pub fn get_transparency(&self) -> f32 {
        use crate::core::offsets::base_part;
        self.memory.read::<f32>(self.address + base_part::transparency())
    }
    
    /// Set the MeshId of a MeshPart.
    ///
    /// Content type layout: instance + MeshId offset → pointer → std::string
    /// MSVC x64 std::string:
    ///   +0x00: union { char buf[16]; char* ptr; }  (SSO / heap)
    ///   +0x10: _Mysize  +0x18: _Myres (<=15 ⇒ SSO)
    ///
    /// Auto-detects pointer indirection vs inline. Allocates if buffer too small.
    pub fn set_mesh_id(&self, mesh_id: &str) {
        use crate::core::memory::is_valid_address;
        use crate::core::offsets::mesh_part;

        let field_addr = self.address + mesh_part::mesh_id();
        let new_len = mesh_id.len();

        let first_qword = self.memory.read::<u64>(field_addr);
        if !is_valid_address(first_qword) {
            tracing::warn!("[COSMETICS] set_mesh_id: invalid value at MeshId offset (0x{:X})", first_qword);
            return;
        }

        // Determine std::string address (try pointer indirection, then inline)
        let string_addr = {
            let maybe_len = self.memory.read::<u64>(first_qword + 0x10);
            let maybe_cap = self.memory.read::<u64>(first_qword + 0x18);
            if maybe_len < 4096 && maybe_cap < 4096 && maybe_cap >= maybe_len {
                first_qword
            } else {
                // Inline string fallback
                let inline_len = self.memory.read::<u64>(field_addr + 0x10);
                let inline_cap = self.memory.read::<u64>(field_addr + 0x18);
                if inline_len < 4096 && inline_cap < 4096 && inline_cap >= inline_len {
                    field_addr
                } else {
                    tracing::warn!(
                        "[COSMETICS] set_mesh_id: cannot find valid string structure (ptr len/cap={}/{}, inline len/cap={}/{})",
                        maybe_len, maybe_cap, inline_len, inline_cap
                    );
                    return;
                }
            }
        };

        let current_len = self.memory.read::<u64>(string_addr + 0x10) as usize;
        let current_cap = self.memory.read::<u64>(string_addr + 0x18) as usize;

        // Buffer too small — allocate new heap buffer
        if current_cap < new_len {
            let alloc_size = new_len + 1; // room for null terminator
            let new_buf = match self.memory.alloc_remote(alloc_size) {
                Some(addr) => addr,
                None => {
                    tracing::warn!("[COSMETICS] set_mesh_id: alloc_remote failed (need {} bytes)", alloc_size);
                    return;
                }
            };

            self.memory.write_bytes(new_buf, mesh_id.as_bytes());
            self.memory.write::<u8>(new_buf + new_len as u64, 0);

            self.memory.write::<u64>(string_addr, new_buf);
            self.memory.write::<u64>(string_addr + 0x10, new_len as u64);
            self.memory.write::<u64>(string_addr + 0x18, new_len as u64);

            tracing::info!("[COSMETICS] set_mesh_id: allocated new buffer at 0x{:X} (cap {} -> {})", new_buf, current_cap, new_len);
            return;
        }

        let data_ptr = self.memory.read::<u64>(string_addr);
        if !is_valid_address(data_ptr) {
            tracing::warn!("[COSMETICS] set_mesh_id: heap data pointer invalid (0x{:X})", data_ptr);
            return;
        }

        // In-place overwrite + null terminator
        self.memory.write_bytes(data_ptr, mesh_id.as_bytes());
        self.memory.write::<u8>(data_ptr + new_len as u64, 0);
        // Zero out any trailing chars from the old string to be safe
        if current_len > new_len {
            let zeroes = vec![0u8; current_len - new_len];
            self.memory.write_bytes(data_ptr + (new_len + 1) as u64, &zeroes);
        }
        self.memory.write::<u64>(string_addr + 0x10, new_len as u64);
    }

    /// Read the current MeshId, handling Content pointer indirection.
    pub fn get_mesh_id(&self) -> String {
        use crate::core::memory::is_valid_address;
        use crate::core::offsets::mesh_part;

        let field_addr = self.address + mesh_part::mesh_id();
        let first_qword = self.memory.read::<u64>(field_addr);
        if !is_valid_address(first_qword) {
            return String::new();
        }

        // Pointer indirection
        let maybe_len = self.memory.read::<u64>(first_qword + 0x10);
        let maybe_cap = self.memory.read::<u64>(first_qword + 0x18);
        if maybe_len < 4096 && maybe_cap < 4096 && maybe_cap >= maybe_len {
            return self.memory.read_string(first_qword);
        }

        // Inline fallback
        self.memory.read_string(field_addr)
    }
}

impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            memory: Arc::clone(&self.memory),
        }
    }
}
