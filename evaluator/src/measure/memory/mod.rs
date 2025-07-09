#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::create_memory_monitor;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::create_memory_monitor;

#[cfg(not(any(target_os = "windows")))]
pub fn create_memory_monitor(_: u32) -> impl FnOnce() -> Option<usize> {
    || None
}
