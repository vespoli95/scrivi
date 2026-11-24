use crate::layout::{
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, PINNED_WINDOW_HEIGHT, PINNED_WINDOW_WIDTH,
};

pub const HWND_TOPMOST_VAL: isize = -1;
pub const HWND_NOTOPMOST_VAL: isize = -2;
pub const SWP_PIN_FLAGS: u32 = 0x0001 | 0x0002 | 0x0010; // SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedWindowBounds {
    pub outer_pos: Option<egui::Pos2>,
    pub inner_size: Option<egui::Vec2>,
    pub maximized: bool,
}

pub fn capture_viewport_bounds(ctx: &egui::Context) -> SavedWindowBounds {
    ctx.input(|i| {
        let vp = i.viewport();
        SavedWindowBounds {
            outer_pos: vp.outer_rect.map(|r| r.min),
            inner_size: vp.inner_rect.map(|r| r.size()),
            maximized: vp.maximized.unwrap_or(false),
        }
    })
}

fn sanitize_scale(pixels_per_point: f32) -> f32 {
    if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    }
}

pub fn pinned_outer_position(work_area: Option<WorkArea>, pixels_per_point: f32) -> egui::Pos2 {
    let scale = sanitize_scale(pixels_per_point);
    match work_area {
        Some(area) => egui::pos2(area.left as f32 / scale, area.top as f32 / scale),
        None => egui::Pos2::ZERO,
    }
}

pub fn pinned_inner_size(work_area: Option<WorkArea>, pixels_per_point: f32) -> egui::Vec2 {
    let scale = sanitize_scale(pixels_per_point);
    let mut width = PINNED_WINDOW_WIDTH;
    let mut height = PINNED_WINDOW_HEIGHT;
    if let Some(area) = work_area {
        let work_w = (area.right - area.left).max(0) as f32 / scale;
        let work_h = (area.bottom - area.top).max(0) as f32 / scale;
        if work_w > 0.0 {
            width = width.min((work_w - 24.0).max(200.0));
        }
        if work_h > 0.0 {
            height = height.min((work_h - 60.0).max(200.0));
        }
    }
    egui::vec2(width, height)
}

pub fn pin_viewport_commands(
    work_area: Option<WorkArea>,
    pixels_per_point: f32,
    was_maximized: bool,
) -> Vec<egui::ViewportCommand> {
    let mut cmds = Vec::with_capacity(4);
    if was_maximized {
        cmds.push(egui::ViewportCommand::Maximized(false));
    }
    cmds.push(egui::ViewportCommand::WindowLevel(
        egui::WindowLevel::AlwaysOnTop,
    ));
    cmds.push(egui::ViewportCommand::InnerSize(pinned_inner_size(
        work_area,
        pixels_per_point,
    )));
    cmds.push(egui::ViewportCommand::OuterPosition(pinned_outer_position(
        work_area,
        pixels_per_point,
    )));
    cmds
}

pub fn unpin_viewport_commands(
    saved: Option<SavedWindowBounds>,
    restored_native_placement: bool,
) -> Vec<egui::ViewportCommand> {
    let mut cmds = Vec::with_capacity(4);
    cmds.push(egui::ViewportCommand::WindowLevel(
        egui::WindowLevel::Normal,
    ));
    if let Some(bounds) = saved {
        if !restored_native_placement {
            if let Some(size) = bounds.inner_size {
                cmds.push(egui::ViewportCommand::InnerSize(size));
            }
            if let Some(pos) = bounds.outer_pos {
                cmds.push(egui::ViewportCommand::OuterPosition(pos));
            }
        }
        if bounds.maximized {
            cmds.push(egui::ViewportCommand::Maximized(true));
        }
    }
    cmds
}

pub fn default_unpinned_bounds(
    work_area: Option<WorkArea>,
    pixels_per_point: f32,
) -> SavedWindowBounds {
    let scale = sanitize_scale(pixels_per_point);
    let mut width = DEFAULT_WINDOW_WIDTH;
    let mut height = DEFAULT_WINDOW_HEIGHT;
    let outer_pos = if let Some(area) = work_area {
        let work_w = (area.right - area.left).max(0) as f32 / scale;
        let work_h = (area.bottom - area.top).max(0) as f32 / scale;
        if work_w > 0.0 {
            width = width.min((work_w - 24.0).max(320.0));
        }
        if work_h > 0.0 {
            height = height.min((work_h - 60.0).max(320.0));
        }
        Some(egui::pos2(
            area.left as f32 / scale + ((work_w - width) * 0.5).max(0.0),
            area.top as f32 / scale + ((work_h - height) * 0.5).max(0.0),
        ))
    } else {
        None
    };
    SavedWindowBounds {
        outer_pos,
        inner_size: Some(egui::vec2(width, height)),
        maximized: false,
    }
}

pub trait WindowPlatform {
    fn supports_pin(&self) -> bool;
    fn set_pinned(&mut self, ctx: &egui::Context, pinned: bool) -> Result<(), String>;
    fn set_pinned_on_startup(&mut self, ctx: &egui::Context) -> Result<(), String> {
        self.set_pinned(ctx, true)
    }
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

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HWND, POINT, RECT};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ,
        REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, IsZoomed, SetWindowPlacement, SetWindowPos, ShowWindow,
        SystemParametersInfoW, SPI_GETWORKAREA, SW_RESTORE, SW_SHOWMAXIMIZED, SW_SHOWNORMAL,
        WINDOWPLACEMENT,
    };

    use super::{
        capture_viewport_bounds, default_unpinned_bounds, pin_viewport_commands,
        set_window_pinned_with, unpin_viewport_commands, SavedWindowBounds, WindowPlatform,
        WorkArea,
    };

    const STARTUP_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const STARTUP_VALUE: &str = "Scrivi";

    struct SavedWindowsState {
        placement: Option<WINDOWPLACEMENT>,
        viewport: SavedWindowBounds,
    }

    pub struct WindowsPlatform {
        hwnd: isize,
        saved_window: Option<SavedWindowsState>,
    }

    impl WindowsPlatform {
        pub fn new(hwnd: isize) -> Self {
            Self {
                hwnd,
                saved_window: None,
            }
        }
    }

    fn query_primary_work_area() -> Option<WorkArea> {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let ok = unsafe {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut rect as *mut RECT).cast(), 0)
        };
        if ok == 0 {
            None
        } else {
            Some(WorkArea {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            })
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

    fn apply_win32_topmost(hwnd: isize, pinned: bool) -> Result<(), String> {
        set_window_pinned_with(
            hwnd,
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
        )
    }

    impl WindowPlatform for WindowsPlatform {
        fn supports_pin(&self) -> bool {
            true
        }

        fn set_pinned_on_startup(&mut self, ctx: &egui::Context) -> Result<(), String> {
            apply_win32_topmost(self.hwnd, true)?;
            let work_area = query_primary_work_area();
            let scale = ctx.pixels_per_point();
            self.saved_window = Some(SavedWindowsState {
                placement: None,
                viewport: default_unpinned_bounds(work_area, scale),
            });
            for cmd in pin_viewport_commands(work_area, scale, false) {
                ctx.send_viewport_cmd(cmd);
            }
            Ok(())
        }

        fn set_pinned(&mut self, ctx: &egui::Context, pinned: bool) -> Result<(), String> {
            apply_win32_topmost(self.hwnd, pinned)?;
            if pinned {
                if self.saved_window.is_none() {
                    let mut viewport = capture_viewport_bounds(ctx);
                    let is_zoomed = unsafe { IsZoomed(self.hwnd as HWND) != 0 };
                    viewport.maximized = viewport.maximized || is_zoomed;
                    let mut wp = WINDOWPLACEMENT {
                        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                        flags: 0,
                        showCmd: 0,
                        ptMinPosition: POINT { x: 0, y: 0 },
                        ptMaxPosition: POINT { x: 0, y: 0 },
                        rcNormalPosition: RECT {
                            left: 0,
                            top: 0,
                            right: 0,
                            bottom: 0,
                        },
                    };
                    let ok = unsafe { GetWindowPlacement(self.hwnd as HWND, &mut wp) };
                    let placement = if ok != 0 { Some(wp) } else { None };
                    self.saved_window = Some(SavedWindowsState {
                        placement,
                        viewport,
                    });
                }
                let was_maximized = self
                    .saved_window
                    .as_ref()
                    .is_some_and(|s| s.viewport.maximized);
                if was_maximized {
                    unsafe {
                        ShowWindow(self.hwnd as HWND, SW_RESTORE);
                    }
                }
                let work_area = query_primary_work_area();
                for cmd in pin_viewport_commands(work_area, ctx.pixels_per_point(), was_maximized) {
                    ctx.send_viewport_cmd(cmd);
                }
            } else {
                let saved = self.saved_window.take();
                let saved_viewport = saved.as_ref().map(|s| s.viewport);
                let mut restored_native = false;
                if let Some(SavedWindowsState {
                    placement: Some(mut wp),
                    viewport,
                }) = saved
                {
                    wp.showCmd = if viewport.maximized {
                        SW_SHOWMAXIMIZED as u32
                    } else {
                        SW_SHOWNORMAL as u32
                    };
                    if unsafe { SetWindowPlacement(self.hwnd as HWND, &wp) } != 0 {
                        restored_native = true;
                    }
                }
                for cmd in unpin_viewport_commands(saved_viewport, restored_native) {
                    ctx.send_viewport_cmd(cmd);
                }
            }
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

    #[test]
    fn test_pinned_outer_position_and_inner_size() {
        let normal_area = Some(WorkArea {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        });
        assert_eq!(pinned_outer_position(normal_area, 1.0), egui::pos2(0.0, 0.0));
        assert_eq!(
            pinned_inner_size(normal_area, 1.0),
            egui::vec2(PINNED_WINDOW_WIDTH, PINNED_WINDOW_HEIGHT)
        );

        let top_taskbar = Some(WorkArea {
            left: 50,
            top: 40,
            right: 1920,
            bottom: 1080,
        });
        assert_eq!(
            pinned_outer_position(top_taskbar, 1.25),
            egui::pos2(40.0, 32.0)
        );
        assert_eq!(pinned_outer_position(None, 1.5), egui::Pos2::ZERO);
        assert_eq!(pinned_outer_position(top_taskbar, 0.0), egui::pos2(50.0, 40.0));

        let tiny_area = Some(WorkArea {
            left: 0,
            top: 0,
            right: 320,
            bottom: 300,
        });
        assert_eq!(pinned_inner_size(tiny_area, 1.0), egui::vec2(296.0, 240.0));
    }

    #[test]
    fn test_pin_and_unpin_viewport_commands() {
        let area = Some(WorkArea {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        });

        let normal_cmds = pin_viewport_commands(area, 1.0, false);
        assert_eq!(normal_cmds.len(), 3);
        assert!(matches!(
            normal_cmds[0],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop)
        ));
        assert!(matches!(
            normal_cmds[1],
            egui::ViewportCommand::InnerSize(s) if s == egui::vec2(PINNED_WINDOW_WIDTH, PINNED_WINDOW_HEIGHT)
        ));
        assert!(matches!(
            normal_cmds[2],
            egui::ViewportCommand::OuterPosition(p) if p == egui::pos2(0.0, 0.0)
        ));

        let max_cmds = pin_viewport_commands(area, 1.0, true);
        assert_eq!(max_cmds.len(), 4);
        assert!(matches!(max_cmds[0], egui::ViewportCommand::Maximized(false)));
        assert!(matches!(
            max_cmds[1],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop)
        ));

        let saved = SavedWindowBounds {
            outer_pos: Some(egui::pos2(200.0, 150.0)),
            inner_size: Some(egui::vec2(760.0, 600.0)),
            maximized: true,
        };
        let native_restore = unpin_viewport_commands(Some(saved), true);
        assert_eq!(native_restore.len(), 2);
        assert!(matches!(
            native_restore[0],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal)
        ));
        assert!(matches!(
            native_restore[1],
            egui::ViewportCommand::Maximized(true)
        ));

        let fallback_restore = unpin_viewport_commands(Some(saved), false);
        assert_eq!(fallback_restore.len(), 4);
        assert!(matches!(
            fallback_restore[0],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal)
        ));
        assert!(matches!(
            fallback_restore[1],
            egui::ViewportCommand::InnerSize(s) if s == egui::vec2(760.0, 600.0)
        ));
        assert!(matches!(
            fallback_restore[2],
            egui::ViewportCommand::OuterPosition(p) if p == egui::pos2(200.0, 150.0)
        ));
        assert!(matches!(
            fallback_restore[3],
            egui::ViewportCommand::Maximized(true)
        ));
    }

    #[test]
    fn test_default_unpinned_bounds_and_restore() {
        let normal_area = Some(WorkArea {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        });
        let bounds = default_unpinned_bounds(normal_area, 1.0);
        assert_eq!(
            bounds,
            SavedWindowBounds {
                outer_pos: Some(egui::pos2(580.0, 220.0)),
                inner_size: Some(egui::vec2(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)),
                maximized: false,
            }
        );

        let restore_cmds = unpin_viewport_commands(Some(bounds), false);
        assert_eq!(restore_cmds.len(), 3);
        assert!(matches!(
            restore_cmds[0],
            egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal)
        ));
        assert!(matches!(
            restore_cmds[1],
            egui::ViewportCommand::InnerSize(s)
                if s == egui::vec2(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)
        ));
        assert!(matches!(
            restore_cmds[2],
            egui::ViewportCommand::OuterPosition(p) if p == egui::pos2(580.0, 220.0)
        ));

        let offset_area = Some(WorkArea {
            left: 50,
            top: 40,
            right: 1920,
            bottom: 1080,
        });
        let scaled_bounds = default_unpinned_bounds(offset_area, 1.25);
        assert_eq!(
            scaled_bounds.inner_size,
            Some(egui::vec2(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT))
        );
        assert_eq!(scaled_bounds.outer_pos, Some(egui::pos2(408.0, 148.0)));

        let no_area = default_unpinned_bounds(None, 1.0);
        assert_eq!(
            no_area,
            SavedWindowBounds {
                outer_pos: None,
                inner_size: Some(egui::vec2(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT)),
                maximized: false,
            }
        );

        let tiny_area = Some(WorkArea {
            left: 0,
            top: 0,
            right: 500,
            bottom: 420,
        });
        let tiny_bounds = default_unpinned_bounds(tiny_area, 1.0);
        assert_eq!(tiny_bounds.inner_size, Some(egui::vec2(476.0, 360.0)));
        assert_eq!(tiny_bounds.outer_pos, Some(egui::pos2(12.0, 30.0)));
    }
}
