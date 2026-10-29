//! 每个交互会话只允许运行一个桌面端实例。

#[cfg(windows)]
mod platform {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::CreateMutexW;

    pub struct SessionInstance(HANDLE);

    impl SessionInstance {
        pub fn acquire() -> Result<Option<Self>, String> {
            let mut session_id = 0_u32;
            unsafe { ProcessIdToSessionId(std::process::id(), &mut session_id) }
                .map_err(|error| format!("读取当前会话失败: {error}"))?;
            let name: Vec<u16> = format!("Local\\AnchorGuard.Desktop.{session_id}")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }
                .map_err(|error| format!("创建桌面端单实例互斥体失败: {error}"))?;
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Ok(None);
            }
            Ok(Some(Self(handle)))
        }
    }

    impl Drop for SessionInstance {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub struct SessionInstance;

    impl SessionInstance {
        pub fn acquire() -> Result<Option<Self>, String> {
            Ok(Some(Self))
        }
    }
}

pub use platform::SessionInstance;
