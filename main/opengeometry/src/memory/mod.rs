#[cfg(test)]
mod tests;

const PAGE_BYTES: usize = 65_536;
const RESERVE_DIVISOR: usize = 8;
const LEAST_RESERVE: usize = 1024 * 1024;
const MOST_RESERVE: usize = 64 * 1024 * 1024;

fn reserve_bytes(pages: usize) -> usize {
    (pages / RESERVE_DIVISOR)
        .saturating_mul(PAGE_BYTES)
        .clamp(LEAST_RESERVE, MOST_RESERVE)
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) use grow_ahead::GrowAhead;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod grow_ahead {
    use super::reserve_bytes;
    use core::arch::wasm32::memory_size;
    use core::sync::atomic::{AtomicBool, Ordering};
    use std::alloc::{GlobalAlloc, Layout, System};

    const RESERVE_ALIGNMENT: usize = 8;

    static RESERVE_OFF: AtomicBool = AtomicBool::new(false);

    pub(crate) struct GrowAhead;

    #[inline(never)]
    unsafe fn system_alloc(layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    #[inline(never)]
    unsafe fn system_dealloc(block: *mut u8, layout: Layout) {
        unsafe { System.dealloc(block, layout) }
    }

    #[inline(never)]
    fn reserve(bytes: usize) {
        let Ok(layout) = Layout::from_size_align(bytes, RESERVE_ALIGNMENT) else {
            return;
        };
        let block = unsafe { system_alloc(layout) };
        if block.is_null() {
            RESERVE_OFF.store(true, Ordering::Relaxed);
        } else {
            unsafe { system_dealloc(block, layout) };
        }
    }

    fn reserve_after_growth(pages_before: usize) {
        let pages = memory_size::<0>();
        if pages != pages_before && !RESERVE_OFF.load(Ordering::Relaxed) {
            reserve(reserve_bytes(pages));
        }
    }

    unsafe impl GlobalAlloc for GrowAhead {
        #[inline(never)]
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pages = memory_size::<0>();
            let block = unsafe { system_alloc(layout) };
            reserve_after_growth(pages);
            block
        }

        #[inline(never)]
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pages = memory_size::<0>();
            let block = unsafe { System.alloc_zeroed(layout) };
            reserve_after_growth(pages);
            block
        }

        #[inline(never)]
        unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
            unsafe { system_dealloc(block, layout) };
        }

        #[inline(never)]
        unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let pages = memory_size::<0>();
            let moved = unsafe { System.realloc(block, layout, new_size) };
            reserve_after_growth(pages);
            moved
        }
    }
}
