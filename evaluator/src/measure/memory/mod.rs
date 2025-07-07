#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::create_memory_monitor;

#[cfg(not(any(target_os = "windows")))]
pub fn create_memory_monitor(_: u32) -> impl FnOnce() -> Option<usize> {
    || None
}
