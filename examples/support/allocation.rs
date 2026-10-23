//! Development-only, current-thread requested-heap accounting.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub calls: u64,
    pub requested: usize,
    pub live: usize,
    pub peak: usize,
    pub accounting_error: bool,
}

thread_local! {
    static STATE: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

fn change(added: usize, removed: usize, allocation: bool) {
    let _ = STATE.try_with(|state| {
        if let Some(mut value) = state.get() {
            value.accounting_error |= removed > value.live;
            value.live = value.live.saturating_sub(removed).saturating_add(added);
            value.peak = value.peak.max(value.live);
            if allocation {
                value.calls = value.calls.saturating_add(1);
                value.requested = value.requested.saturating_add(added);
            }
            state.set(Some(value));
        }
    });
}

pub struct Counting;

// SAFETY: every operation delegates unchanged pointers/layouts to System.
// Accounting uses allocation-free thread-local Cells and never inspects memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            change(layout.size(), 0, true);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            change(layout.size(), 0, true);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        change(0, layout.size(), false);
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            change(size, layout.size(), true);
        }
        next
    }
}

pub fn snapshot() -> Snapshot {
    STATE.with(|state| state.get().expect("measurement not active"))
}

pub fn measure<T>(run: impl FnOnce() -> T) -> (T, Snapshot) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            STATE.with(|state| state.set(None));
        }
    }
    STATE.with(|state| {
        assert!(state.get().is_none());
        state.set(Some(Snapshot::default()));
    });
    let reset = Reset;
    let result = run();
    let measured = snapshot();
    drop(reset);
    (result, measured)
}
