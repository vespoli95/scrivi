pub const HWND_TOPMOST_VAL: isize = -1;
pub const HWND_NOTOPMOST_VAL: isize = -2;
pub const SWP_PIN_FLAGS: u32 = 0x0001 | 0x0002 | 0x0010; // SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE

pub trait WindowPlatform {
    fn supports_pin(&self) -> bool;
    fn set_pinned(&mut self, ctx: &egui::Context, pinned: bool) -> Result<(), String>;
    fn supports_startup(&self) -> bool;
    fn startup_enabled(&mut self) -> Result<bool, String>;
    fn set_startup(&mut self, enabled: bool) -> Result<(), String>;
}

pub fn set_window_pinned_with<F>(
    hwnd: isize,
    pinned: bool,
    set_window_pos: F,
) -> Result<(), String>
where
    F: FnOnce(isize, isize, i32, i32, i32, i32, u32) -> Result<(), String>,
{
    if hwnd == 0 {
        return Err("pin window: native window is unavailable".into());
    }
    let insert_after = if pinned {
        HWND_TOPMOST_VAL
    } else {
        HWND_NOTOPMOST_VAL
    };
    set_window_pos(hwnd, insert_after, 0, 0, 0, 0, SWP_PIN_FLAGS)
}

#[cfg(not(windows))]
#[derive(Default)]
pub struct DefaultPlatform;

#[cfg(not(windows))]
impl WindowPlatform for DefaultPlatform {
    fn supports_pin(&self) -> bool {
        false
    }

    fn set_pinned(&mut self, _ctx: &egui::Context, _pinned: bool) -> Result<(), String> {
        Err("pin window: unsupported on this platform".into())
    }

    fn supports_startup(&self) -> bool {
        false
    }

    fn startup_enabled(&mut self) -> Result<bool, String> {
        Ok(false)
    }

    fn set_startup(&mut self, _enabled: bool) -> Result<(), String> {
        Err("startup setting: unsupported on this platform".into())
    }
}

#[cfg(windows)]
pub use windows_impl::WindowsPlatform;

#[cfg(windows)]
mod windows_impl {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ,
        REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SetWindowPos;

    use super::{set_window_pinned_with, WindowPlatform};

    const STARTUP_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const STARTUP_VALUE: &str = "Scrivi";

    pub struct WindowsPlatform {
        hwnd: isize,
    }

    impl WindowsPlatform {
        pub fn new(hwnd: isize) -> Self {
            Self { hwnd }
        }
    }

    struct RegKeyGuard(HKEY);

    impl Drop for RegKeyGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    RegCloseKey(self.0);
                }
            }
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn format_win32_error(prefix: &str, code: u32) -> String {
        let err = std::io::Error::from_raw_os_error(code as i32);
        format!("{prefix}: {err}")
    }

    impl WindowPlatform for WindowsPlatform {
        fn supports_pin(&self) -> bool {
            true
        }

        fn set_pinned(&mut self, ctx: &egui::Context, pinned: bool) -> Result<(), String> {
            set_window_pinned_with(
                self.hwnd,
                pinned,
                |hwnd, insert_after, x, y, cx, cy, flags| {
                    let ok = unsafe {
                        SetWindowPos(
                            hwnd as HWND,
                            insert_after as HWND,
                            x,
                            y,
                            cx,
                            cy,
                            flags,
                        )
                    };
                    if ok == 0 {
                        let err = std::io::Error::last_os_error();
                        if err.raw_os_error() == Some(0) {
                            return Err("pin window: SetWindowPos failed".into());
                        }
                        return Err(format!("pin window: {err}"));
                    }
                    Ok(())
                },
            )?;
            let level = if pinned {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            Ok(())
        }

        fn supports_startup(&self) -> bool {
            true
        }

        fn startup_enabled(&mut self) -> Result<bool, String> {
            let subkey = to_wide(STARTUP_KEY);
            let value_name = to_wide(STARTUP_VALUE);
            let mut key: HKEY = ptr::null_mut();
            let status = unsafe {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    KEY_QUERY_VALUE,
                    &mut key,
                )
            };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(false);
            }
            if status != ERROR_SUCCESS {
                return Err(format_win32_error("read startup setting", status));
            }
            let _guard = RegKeyGuard(key);

            let mut value_type: REG_VALUE_TYPE = 0;
            let mut data_len: u32 = 0;
            let status = unsafe {
                RegQueryValueExW(
                    key,
                    value_name.as_ptr(),
                    ptr::null(),
                    &mut value_type,
                    ptr::null_mut(),
                    &mut data_len,
                )
            };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(false);
            }
            if status != ERROR_SUCCESS {
                return Err(format_win32_error("read startup setting", status));
            }
            if value_type != REG_SZ && value_type != REG_EXPAND_SZ {
                return Err("read startup setting: unexpected registry value type".into());
            }
            Ok(true)
        }

        fn set_startup(&mut self, enabled: bool) -> Result<(), String> {
            let subkey = to_wide(STARTUP_KEY);
            let value_name = to_wide(STARTUP_VALUE);
            if !enabled {
                let mut key: HKEY = ptr::null_mut();
                let status = unsafe {
                    RegOpenKeyExW(
                        HKEY_CURRENT_USER,
                        subkey.as_ptr(),
                        0,
                        KEY_SET_VALUE,
                        &mut key,
                    )
                };
                if status == ERROR_FILE_NOT_FOUND {
                    return Ok(());
                }
                if status != ERROR_SUCCESS {
                    return Err(format_win32_error("disable startup", status));
                }
                let _guard = RegKeyGuard(key);
                let status = unsafe { RegDeleteValueW(key, value_name.as_ptr()) };
                if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
                    return Err(format_win32_error("disable startup", status));
                }
                return Ok(());
            }

            let executable = std::env::current_exe()
                .map_err(|e| format!("find executable for startup: {e}"))?;
            let command = format!("\"{}\"", executable.display());
            let command_wide = to_wide(&command);
            let byte_len = u32::try_from(command_wide.len() * std::mem::size_of::<u16>())
                .map_err(|_| "enable startup: executable path too long".to_string())?;

            let mut key: HKEY = ptr::null_mut();
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    0,
                    ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    ptr::null(),
                    &mut key,
                    ptr::null_mut(),
                )
            };
            if status != ERROR_SUCCESS {
                return Err(format_win32_error("enable startup", status));
            }
            let _guard = RegKeyGuard(key);

            let status = unsafe {
                RegSetValueExW(
                    key,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    command_wide.as_ptr() as *const u8,
                    byte_len,
                )
            };
            if status != ERROR_SUCCESS {
                return Err(format_win32_error("enable startup", status));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_window_pinned() {
        let mut captured: Option<(isize, isize, u32)> = None;
        for (pinned, expected_after) in [(true, HWND_TOPMOST_VAL), (false, HWND_NOTOPMOST_VAL)] {
            set_window_pinned_with(123, pinned, |hwnd, after, x, y, cx, cy, flags| {
                assert_eq!(x, 0);
                assert_eq!(y, 0);
                assert_eq!(cx, 0);
                assert_eq!(cy, 0);
                captured = Some((hwnd, after, flags));
                Ok(())
            })
            .unwrap();
            assert_eq!(captured, Some((123, expected_after, 0x13)));
        }

        let err = set_window_pinned_with(123, true, |_, _, _, _, _, _, _| {
            Err("access denied".into())
        })
        .unwrap_err();
        assert_eq!(err, "access denied");

        assert!(set_window_pinned_with(0, true, |_, _, _, _, _, _, _| Ok(())).is_err());
    }
}
