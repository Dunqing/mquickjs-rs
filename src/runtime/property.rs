//! Property table and operations
//!
//! JavaScript objects store properties in a hash table for fast lookup.
//! This module implements the property storage and access operations.

use crate::runtime::object::{Property, PropertyType};
use crate::value::Value;

/// Property table structure
///
/// Layout in memory:
/// - prop_count: number of properties (excluding deleted)
/// - hash_mask: hash table size - 1
/// - hash_table[hash_size]: indices into props array (0 = end of list)
/// - props[]: Property array
///
/// This structure is stored in a JSValueArray allocation.
#[derive(Debug)]
pub struct PropertyTable {
    /// Number of active properties
    prop_count: u32,
    /// Hash table mask (size - 1)
    hash_mask: u32,
    /// Properties
    properties: Vec<Property>,
    /// Hash table (indices into properties, 0 = end of chain)
    hash_table: Vec<u32>,
    /// First free slot in properties (for reuse of deleted slots)
    first_free: u32,
}

impl PropertyTable {
    /// Minimum hash table size
    const MIN_HASH_SIZE: usize = 4;

    /// Maximum load factor before resize
    const MAX_LOAD_FACTOR: f64 = 0.75;

    /// Create a new empty property table
    pub fn new() -> Self {
        PropertyTable {
            prop_count: 0,
            hash_mask: (Self::MIN_HASH_SIZE - 1) as u32,
            properties: Vec::new(),
            hash_table: vec![0; Self::MIN_HASH_SIZE],
            first_free: 0,
        }
    }

    /// Create a property table with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let hash_size = capacity.next_power_of_two().max(Self::MIN_HASH_SIZE);
        PropertyTable {
            prop_count: 0,
            hash_mask: (hash_size - 1) as u32,
            properties: Vec::with_capacity(capacity),
            hash_table: vec![0; hash_size],
            first_free: 0,
        }
    }

    /// Get the number of properties
    #[inline]
    pub fn len(&self) -> usize {
        self.prop_count as usize
    }

    /// Check if the table is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.prop_count == 0
    }

    /// Hash a property key
    #[inline]
    fn hash_key(key: Value) -> u32 {
        // Use the raw value as hash (works for both integers and pointers)
        let raw = key.raw().0;
        // Mix bits for better distribution
        let mut h = raw as u32;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85ebca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2ae35);
        h ^= h >> 16;
        h
    }

    /// Find a property by key
    ///
    /// Returns the property index if found.
    pub fn find(&self, key: Value) -> Option<usize> {
        if self.prop_count == 0 {
            return None;
        }

        let hash = Self::hash_key(key);
        let mut idx = self.hash_table[(hash & self.hash_mask) as usize];

        while idx != 0 {
            let prop_idx = (idx - 1) as usize;
            let prop = &self.properties[prop_idx];

            // Check if key matches
            if prop.key == key {
                return Some(prop_idx);
            }

            idx = prop.hash_next();
        }

        None
    }

    /// Get a property value by key
    pub fn get(&self, key: Value) -> Option<&Property> {
        self.find(key).map(|idx| &self.properties[idx])
    }

    /// Get a mutable property reference by key
    pub fn get_mut(&mut self, key: Value) -> Option<&mut Property> {
        self.find(key).map(move |idx| &mut self.properties[idx])
    }

    /// Insert or update a property
    ///
    /// Returns true if this was a new property, false if updated.
    pub fn set(&mut self, key: Value, value: Value) -> bool {
        // Check if property already exists
        if let Some(idx) = self.find(key) {
            self.properties[idx].value = value;
            return false;
        }

        // Check if we need to resize
        let load = (self.properties.len() + 1) as f64 / (self.hash_mask + 1) as f64;
        if load > Self::MAX_LOAD_FACTOR {
            self.resize();
        }

        // Insert new property
        let hash = Self::hash_key(key);
        let bucket = (hash & self.hash_mask) as usize;

        let mut prop = Property::new(key, value);
        prop.set_hash_next(self.hash_table[bucket]);

        // Reuse deleted slot or append
        let prop_idx = if self.first_free != 0 {
            let idx = (self.first_free - 1) as usize;
            self.first_free = self.properties[idx].hash_next();
            self.properties[idx] = prop;
            idx
        } else {
            let idx = self.properties.len();
            self.properties.push(prop);
            idx
        };

        self.hash_table[bucket] = (prop_idx + 1) as u32;
        self.prop_count += 1;

        true
    }

    /// Delete a property by key
    ///
    /// Returns true if the property existed.
    pub fn delete(&mut self, key: Value) -> bool {
        if self.prop_count == 0 {
            return false;
        }

        let hash = Self::hash_key(key);
        let bucket = (hash & self.hash_mask) as usize;

        let mut prev_idx: Option<usize> = None;
        let mut idx = self.hash_table[bucket];

        while idx != 0 {
            let prop_idx = (idx - 1) as usize;
            let prop = &self.properties[prop_idx];

            if prop.key == key {
                // Found it - remove from hash chain
                let next = prop.hash_next();

                if let Some(prev) = prev_idx {
                    self.properties[prev].set_hash_next(next);
                } else {
                    self.hash_table[bucket] = next;
                }

                // Mark as deleted by putting on free list
                self.properties[prop_idx].key = Value::uninitialized();
                self.properties[prop_idx].set_hash_next(self.first_free);
                self.first_free = (prop_idx + 1) as u32;

                self.prop_count -= 1;
                return true;
            }

            prev_idx = Some(prop_idx);
            idx = prop.hash_next();
        }

        false
    }

    /// Resize the hash table
    fn resize(&mut self) {
        let new_size = ((self.hash_mask + 1) * 2) as usize;
        self.hash_mask = (new_size - 1) as u32;
        self.hash_table = vec![0; new_size];

        // Rehash all properties
        for i in 0..self.properties.len() {
            let prop = &self.properties[i];

            // Skip deleted entries
            if prop.key.is_uninitialized() {
                continue;
            }

            let hash = Self::hash_key(prop.key);
            let bucket = (hash & self.hash_mask) as usize;

            self.properties[i].set_hash_next(self.hash_table[bucket]);
            self.hash_table[bucket] = (i + 1) as u32;
        }
    }

    /// Iterate over all properties
    pub fn iter(&self) -> impl Iterator<Item = &Property> {
        self.properties.iter().filter(|p| !p.key.is_uninitialized())
    }

    /// Iterate over all property keys
    pub fn keys(&self) -> impl Iterator<Item = Value> + '_ {
        self.iter().map(|p| p.key)
    }

    /// Check if a property exists
    pub fn has(&self, key: Value) -> bool {
        self.find(key).is_some()
    }

    /// Define a property with getter/setter
    pub fn define_accessor(&mut self, key: Value, getter: Value, setter: Value) -> bool {
        // For getter/setter, value is an array [getter, setter]
        // This is a simplified version - full implementation would allocate array
        let _ = (getter, setter);
        if let Some(idx) = self.find(key) {
            self.properties[idx].set_prop_type(PropertyType::GetSet);
            return false;
        }

        let mut prop = Property::new(key, Value::null());
        prop.set_prop_type(PropertyType::GetSet);
        self.properties.push(prop);
        self.prop_count += 1;
        true
    }
}

impl Default for PropertyTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_table() {
        let table = PropertyTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert!(table.get(Value::int(1)).is_none());
    }

    #[test]
    fn test_set_get() {
        let mut table = PropertyTable::new();

        let key = Value::int(42);
        let value = Value::int(100);

        assert!(table.set(key, value));
        assert!(!table.is_empty());
        assert_eq!(table.len(), 1);

        let prop = table.get(key).unwrap();
        assert_eq!(prop.value, value);
    }

    #[test]
    fn test_update() {
        let mut table = PropertyTable::new();

        let key = Value::int(1);
        table.set(key, Value::int(10));
        table.set(key, Value::int(20));

        assert_eq!(table.len(), 1);
        assert_eq!(table.get(key).unwrap().value, Value::int(20));
    }

    #[test]
    fn test_delete() {
        let mut table = PropertyTable::new();

        let key = Value::int(1);
        table.set(key, Value::int(10));
        assert!(table.has(key));

        assert!(table.delete(key));
        assert!(!table.has(key));
        assert!(table.is_empty());

        assert!(!table.delete(key)); // Already deleted
    }

    #[test]
    fn test_multiple_properties() {
        let mut table = PropertyTable::new();

        for i in 0..100 {
            table.set(Value::int(i), Value::int(i * 2));
        }

        assert_eq!(table.len(), 100);

        for i in 0..100 {
            let prop = table.get(Value::int(i)).unwrap();
            assert_eq!(prop.value, Value::int(i * 2));
        }
    }

    #[test]
    fn test_resize() {
        let mut table = PropertyTable::with_capacity(4);

        // Insert enough to trigger resize
        for i in 0..20 {
            table.set(Value::int(i), Value::int(i));
        }

        // All should still be findable
        for i in 0..20 {
            assert!(table.has(Value::int(i)));
        }
    }

    #[test]
    fn test_delete_and_reuse() {
        let mut table = PropertyTable::new();

        table.set(Value::int(1), Value::int(10));
        table.set(Value::int(2), Value::int(20));
        table.set(Value::int(3), Value::int(30));

        table.delete(Value::int(2));
        assert_eq!(table.len(), 2);

        // New property should reuse deleted slot
        table.set(Value::int(4), Value::int(40));
        assert_eq!(table.len(), 3);

        assert!(table.has(Value::int(1)));
        assert!(!table.has(Value::int(2)));
        assert!(table.has(Value::int(3)));
        assert!(table.has(Value::int(4)));
    }

    #[test]
    fn test_keys_iterator() {
        let mut table = PropertyTable::new();

        table.set(Value::int(1), Value::int(10));
        table.set(Value::int(2), Value::int(20));
        table.set(Value::int(3), Value::int(30));

        let keys: Vec<_> = table.keys().collect();
        assert_eq!(keys.len(), 3);
    }
}
