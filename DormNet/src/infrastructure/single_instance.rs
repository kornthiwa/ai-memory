//! Ensure only one DormNet process runs at a time (Windows named mutex).

use anyhow::{bail, Result};

#[cfg(windows)]
mod win {
    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    pub struct InstanceLock {
        handle: HANDLE,
    }

    impl InstanceLock {
        pub fn acquire(name: &str) -> Result<Self> {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
            if handle.is_null() {
                bail!("CreateMutexW failed: {}", unsafe { GetLastError() });
            }
            let err = unsafe { GetLastError() };
            if err == ERROR_ALREADY_EXISTS {
                unsafe {
                    CloseHandle(handle);
                }
                bail!("another DormNet instance is already running");
            }
            Ok(Self { handle })
        }
    }

    impl Drop for InstanceLock {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(windows)]
pub use win::InstanceLock;

#[cfg(not(windows))]
pub struct InstanceLock;

#[cfg(not(windows))]
impl InstanceLock {
    pub fn acquire(_name: &str) -> Result<Self> {
        Ok(Self)
    }
}

pub const DEFAULT_MUTEX_NAME: &str = "Local\\DormNetSingleInstance";

pub fn acquire_default() -> Result<InstanceLock> {
    InstanceLock::acquire(DEFAULT_MUTEX_NAME)
}

#[allow(dead_code)]
fn _keep_bail_used() -> Result<()> {
    if false {
        bail!("unreachable");
    }
    Ok(())
}
