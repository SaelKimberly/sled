use core::fmt;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ptr;

/// Number of words a piece of `Data` can hold.
///
/// Three words should be enough for the majority of cases. For example, you can fit inside it the
/// function pointer together with a fat pointer representing an object that needs to be destroyed.
const DATA_WORDS: usize = 3;

/// Some space to keep a `FnOnce()` object on the stack.
type Data = MaybeUninit<[usize; DATA_WORDS]>;

/// A `FnOnce()` that is stored inline if small, or otherwise boxed on the heap.
///
/// This is a handy way of keeping an unsized `FnOnce()` within a sized structure.
pub(super) struct Deferred {
    call: unsafe fn(*mut u8),
    data: Data,
    _marker: PhantomData<*mut ()>, // !Send + !Sync
}

impl fmt::Debug for Deferred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.pad("Deferred { .. }")
    }
}

unsafe fn call_raw<F: FnOnce()>(raw: *mut u8) {
    unsafe {
        let f: F = ptr::read(raw as *mut F);
        f();
    }
}

unsafe fn call_raw_box<F: FnOnce()>(raw: *mut u8) {
    unsafe {
        // It's safe to cast `raw` from `*mut u8` to `*mut Box<F>`, because `raw` is
        // originally derived from `*mut Box<F>`.
        #[allow(clippy::cast_ptr_alignment)]
        let b: Box<F> = ptr::read(raw as *mut Box<F>);
        (*b)();
    }
}

impl Deferred {
    /// Constructs a new `Deferred` from a `FnOnce()`.
    pub(super) fn new<F: FnOnce()>(f: F) -> Self {
        let size = mem::size_of::<F>();
        let align = mem::align_of::<F>();

        unsafe {
            if size <= mem::size_of::<Data>()
                && align <= mem::align_of::<Data>()
            {
                let mut data = Data::uninit();
                ptr::write(data.as_mut_ptr() as *mut F, f);

                Deferred { call: call_raw::<F>, data, _marker: PhantomData }
            } else {
                let b: Box<F> = Box::new(f);
                let mut data = Data::uninit();
                ptr::write(data.as_mut_ptr() as *mut Box<F>, b);

                Deferred { call: call_raw_box::<F>, data, _marker: PhantomData }
            }
        }
    }

    /// Calls the function.
    #[inline]
    pub(super) fn call(mut self) {
        let call = self.call;
        #[allow(trivial_casts)]
        unsafe {
            call(self.data.as_mut_ptr().cast())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::Deferred;
    use std::cell::Cell;

    #[test]
    #[allow(dropping_copy_types)]
    fn on_stack() {
        let fired = &Cell::new(false);
        let a = [0_usize; 1];

        let d = Deferred::new(move || {
            drop(a);
            fired.set(true);
        });

        assert!(!fired.get());
        d.call();
        assert!(fired.get());
    }

    #[test]
    #[allow(dropping_copy_types)]
    fn on_heap() {
        let fired = &Cell::new(false);
        let a = [0_usize; 10];

        let d = Deferred::new(move || {
            drop(a);
            fired.set(true);
        });

        assert!(!fired.get());
        d.call();
        assert!(fired.get());
    }

    #[test]
    fn string() {
        let a = "hello".to_string();
        let d = Deferred::new(move || assert_eq!(a, "hello"));
        d.call();
    }

    #[test]
    fn boxed_slice_i32() {
        let a: Box<[i32]> = vec![2, 3, 5, 7].into_boxed_slice();
        let d = Deferred::new(move || assert_eq!(*a, [2, 3, 5, 7]));
        d.call();
    }

    #[test]
    fn long_slice_usize() {
        let a: [usize; 5] = [2, 3, 5, 7, 11];
        let d = Deferred::new(move || assert_eq!(a, [2, 3, 5, 7, 11]));
        d.call();
    }
}
