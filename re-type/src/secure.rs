// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Typing buffer that is wiped on every exit path (REQ-SEC-003).

use std::sync::atomic::{compiler_fence, Ordering};

/// Fixed-capacity UTF-16 buffer. It is allocated once and never grows, so no stale copy of
/// the text is left behind by a reallocation. `clear` overwrites the whole storage with
/// volatile writes that the optimiser may not remove, and `Drop` calls it, so completed,
/// aborted, failed and panicking runs all end with a wiped buffer.
pub struct SecureBuf {
    data: Box<[u16]>,
    len: usize,
}

impl SecureBuf {
    pub fn with_capacity(capacity: usize) -> Self {
        SecureBuf { data: vec![0u16; capacity].into_boxed_slice(), len: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_units(&self) -> &[u16] {
        &self.data[..self.len]
    }

    /// Mutable view of the whole storage, for the platform layer to copy into directly.
    /// Call `set_len` afterwards.
    pub fn storage_mut(&mut self) -> &mut [u16] {
        &mut self.data
    }

    /// Records how many units of the storage are valid.
    pub fn set_len(&mut self, len: usize) {
        assert!(len <= self.data.len());
        self.len = len;
    }

    /// Copies `src` in. Returns `Err(src.len())` if it does not fit; the buffer is then empty.
    pub fn fill_from(&mut self, src: &[u16]) -> Result<(), usize> {
        self.clear();
        if src.len() > self.data.len() {
            return Err(src.len());
        }
        self.data[..src.len()].copy_from_slice(src);
        self.len = src.len();
        Ok(())
    }

    pub fn clear(&mut self) {
        for unit in self.data.iter_mut() {
            // SAFETY: `unit` is a valid, aligned, exclusive reference.
            unsafe { std::ptr::write_volatile(unit, 0) };
        }
        compiler_fence(Ordering::SeqCst);
        self.len = 0;
    }

    /// Test helper: true if every unit of the storage is zero.
    pub fn is_wiped(&self) -> bool {
        self.data.iter().all(|&u| u == 0)
    }
}

impl Drop for SecureBuf {
    fn drop(&mut self) {
        self.clear();
    }
}

impl std::fmt::Debug for SecureBuf {
    // Never prints the content.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecureBuf {{ len: {}, capacity: {} }}", self.len, self.data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_read_clear() {
        let mut b = SecureBuf::with_capacity(8);
        b.fill_from(&[1, 2, 3]).unwrap();
        assert_eq!(b.as_units(), &[1, 2, 3]);
        b.clear();
        assert!(b.is_empty() && b.is_wiped());
    }

    #[test]
    fn too_long_is_refused_and_leaves_buffer_empty() {
        let mut b = SecureBuf::with_capacity(2);
        assert_eq!(b.fill_from(&[1, 2, 3]), Err(3));
        assert!(b.is_empty() && b.is_wiped());
    }

    #[test]
    fn debug_does_not_leak_content() {
        let mut b = SecureBuf::with_capacity(4);
        b.fill_from(&[0x73, 0x65, 0x63]).unwrap();
        let s = format!("{b:?}");
        assert!(!s.contains("115") && s.contains("len: 3"));
    }
}
