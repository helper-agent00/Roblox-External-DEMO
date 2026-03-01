#![allow(dead_code)]

use std::sync::Arc;

use crate::core::memory::{is_valid_address, Memory};
use crate::core::offsets::{base_part, primitive, primitive_flags};
use crate::sdk::Instance;
use crate::utils::math::{Matrix3, Vector3};

/// Represents a BasePart (body parts, terrain, etc.).
pub struct Part {
    instance: Instance,
}

impl Part {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self {
            instance: Instance::new(address, memory),
        }
    }

    #[inline]
    pub fn address(&self) -> u64 {
        self.instance.address
    }

    pub fn get_primitive(&self) -> Primitive {
        let prim_addr = self.instance.memory().read::<u64>(self.instance.address + base_part::primitive());
        Primitive::new(prim_addr, Arc::clone(self.instance.memory()))
    }

    pub fn get_position(&self) -> Vector3 {
        self.instance.memory().read::<Vector3>(self.instance.address + base_part::position())
    }

    pub fn get_size(&self) -> Vector3 {
        self.instance.memory().read::<Vector3>(self.instance.address + base_part::size())
    }

    pub fn get_transparency(&self) -> f32 {
        self.instance.memory().read::<f32>(self.instance.address + base_part::transparency())
    }

    pub fn get_flags(&self) -> u8 {
        self.instance.memory().read::<u8>(self.instance.address + base_part::primitive_flags())
    }

    pub fn set_flags(&self, flags: u8) {
        self.instance.memory().write::<u8>(self.instance.address + base_part::primitive_flags(), flags);
    }

    pub fn can_collide(&self) -> bool {
        (self.get_flags() & primitive_flags::can_collide() as u8) != 0
    }

    pub fn set_can_collide(&self, can_collide: bool) {
        let flags = self.get_flags();
        let new_flags = if can_collide {
            flags | primitive_flags::can_collide() as u8
        } else {
            flags & !(primitive_flags::can_collide() as u8)
        };
        self.set_flags(new_flags);
    }

    pub fn is_anchored(&self) -> bool {
        (self.get_flags() & primitive_flags::anchored() as u8) != 0
    }
}

/// Represents a Primitive (physics body).
pub struct Primitive {
    pub address: u64,
    memory: Arc<Memory>,
}

impl Primitive {
    pub fn new(address: u64, memory: Arc<Memory>) -> Self {
        Self { address, memory }
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        is_valid_address(self.address)
    }

    pub fn get_position(&self) -> Vector3 {
        if !self.is_valid() {
            return Vector3::ZERO;
        }
        self.memory.read::<Vector3>(self.address + base_part::position())
    }

    pub fn set_position(&self, pos: Vector3) {
        if self.is_valid() {
            self.memory.write::<Vector3>(self.address + base_part::position(), pos);
        }
    }

    pub fn get_velocity(&self) -> Vector3 {
        if !self.is_valid() {
            return Vector3::ZERO;
        }
        self.memory.read::<Vector3>(self.address + primitive::velocity())
    }

    pub fn set_velocity(&self, vel: Vector3) {
        if self.is_valid() {
            self.memory.write::<Vector3>(self.address + primitive::velocity(), vel);
        }
    }

    pub fn get_rotation(&self) -> Matrix3 {
        if !self.is_valid() {
            return Matrix3::default();
        }
        self.memory.read::<Matrix3>(self.address + base_part::rotation())
    }

    pub fn get_size(&self) -> Vector3 {
        if !self.is_valid() {
            return Vector3::ZERO;
        }
        self.memory.read::<Vector3>(self.address + base_part::size())
    }
}
