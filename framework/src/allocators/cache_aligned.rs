use std::alloc::{self, alloc_zeroed, Layout};
use std::fmt;
use std::mem::{align_of, size_of};
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};

const CACHE_LINE_SIZE: usize = 64;

#[inline]
fn cache_aligned_layout_for<T>() -> Layout {
    // Ensure the alignment satisfies both cache line size and T's natural alignment
    let align = CACHE_LINE_SIZE.max(align_of::<T>());
    Layout::from_size_align(size_of::<T>(), align).expect("invalid layout")
}

unsafe fn allocate_cache_aligned<T>() -> *mut T { unsafe {
    let layout = cache_aligned_layout_for::<T>();
    let ptr = alloc_zeroed(layout) as *mut T;
    if ptr.is_null() {
        alloc::handle_alloc_error(layout);
    }
    ptr
} }

#[derive(Debug)]
pub struct CacheAligned<T: Sized> {
    ptr: NonNull<T>,
}

// It is safe to mark CacheAligned<T> as Send/Sync when T has the
// corresponding auto-traits, since the wrapper only owns a uniquely
// allocated instance of T and does not introduce interior mutability
// beyond T itself.
unsafe impl<T: Send> Send for CacheAligned<T> {}
unsafe impl<T: Sync> Sync for CacheAligned<T> {}

impl<T: Sized> Drop for CacheAligned<T> {
    fn drop(&mut self) {
        unsafe {
            // Drop the contained value first
            ptr::drop_in_place(self.ptr.as_ptr());
            // Then deallocate the memory with the same layout
            alloc::dealloc(
                self.ptr.as_ptr() as *mut u8,
                cache_aligned_layout_for::<T>(),
            );
        }
    }
}

impl<T: Sized> Deref for CacheAligned<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: Sized> DerefMut for CacheAligned<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: Sized> CacheAligned<T> {
    pub fn allocate(src: T) -> CacheAligned<T> {
        unsafe {
            let alloc = allocate_cache_aligned::<T>();
            ptr::write(alloc, src);
            CacheAligned {
                ptr: NonNull::new_unchecked(alloc),
            }
        }
    }
}

impl<T: Sized> Clone for CacheAligned<T>
where
    T: Clone,
{
    fn clone(&self) -> CacheAligned<T> {
        unsafe {
            let alloc = allocate_cache_aligned::<T>();
            // Use T::clone to duplicate the inner value, not the wrapper
            ptr::write(alloc, self.deref().clone());
            CacheAligned {
                ptr: NonNull::new_unchecked(alloc),
            }
        }
    }
}

impl<T: Sized> fmt::Display for CacheAligned<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        T::fmt(&*self, f)
    }
}
