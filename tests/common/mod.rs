#[cfg(not(any(feature = "testing", feature = "light_testing")))]
compile_error!(
    "please run tests using the \"testing\" feature, \
     which enables additional checks at runtime and \
     causes more race conditions to jump out by \
     inserting delays in concurrent code."
);

// the memshred feature causes all allocated and deallocated
// memory to be set to a specific non-zero value of 0xa1 for
// uninitialized allocations and 0xde for deallocated memory,
// in the hope that it will cause memory errors to surface
// more quickly.
#[cfg(feature = "memshred")]
mod alloc {
    use std::alloc::{Layout, System};

    #[global_allocator]
    static ALLOCATOR: Alloc = Alloc;

    #[derive(Default, Debug, Clone, Copy)]
    struct Alloc;

    unsafe impl std::alloc::GlobalAlloc for Alloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let ret = unsafe { System.alloc(layout) };
            assert_ne!(ret, std::ptr::null_mut());
            unsafe {
                std::ptr::write_bytes(ret, 0xa1, layout.size());
            }
            ret
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe {
                std::ptr::write_bytes(ptr, 0xde, layout.size());
                System.dealloc(ptr, layout)
            }
        }
    }
}

pub fn setup_logger() {
    use std::io::Write;

    fn tn() -> String {
        std::thread::current().name().unwrap_or("unknown").to_owned()
    }

    #[cfg(feature = "pretty_backtrace")]
    color_backtrace::install();

    let mut builder = env_logger::Builder::new();
    builder
        .format(|buf, record| {
            writeln!(
                buf,
                "{:05} {:20} {:10} {}",
                record.level(),
                tn(),
                record.module_path().unwrap().split("::").last().unwrap(),
                record.args()
            )
        })
        .filter(None, log::LevelFilter::Info);

    if let Ok(env) = std::env::var("RUST_LOG") {
        builder.parse_filters(&env);
    }

    let _r = builder.try_init();
}

#[allow(dead_code)]
pub fn cleanup(dir: &str) {
    let dir = std::path::Path::new(dir);
    if dir.exists() {
        std::fs::remove_dir_all(dir).unwrap();
    }
}
