//! Windows-specific resize handling for frameless windows.
//! This module hooks into Windows messages to enable resize from edges/corners.
//! Using a custom title bar with `decorations(false)` disables the default OS resize behavior, so we have to implement it ourselves.
//! I think this is only a problem on Windows but I have not tested on other platforms of course.
//! WHYYYYYYYY

use raw_window_handle::HasWindowHandle;

#[cfg(target_os = "windows")]
/// Takes our window handle from eframe and sets up a custom window procedure to handle WM_NCHITTEST messages for resizing the borderless window.
pub fn setup_borderless_resize<W: HasWindowHandle>(window: &W) {
    use raw_window_handle::RawWindowHandle;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, GWLP_WNDPROC, GetWindowLongPtrW, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT,
        HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, SetWindowLongPtrW,
        WM_NCHITTEST,
    };

    // Store original window procedures for each window
    static ORIGINAL_WNDPROCS: Mutex<Option<HashMap<isize, isize>>> = Mutex::new(None);

    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            unsafe {
                let hwnd = win32_handle.hwnd.get() as HWND;

                // Save the original window procedure
                let original_wndproc = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
                {
                    let mut map = ORIGINAL_WNDPROCS.lock().unwrap();
                    if map.is_none() {
                        *map = Some(HashMap::new());
                    }
                    map.as_mut()
                        .unwrap()
                        .insert(hwnd as isize, original_wndproc);
                }

                // Custom window procedure that handles hit testing
                unsafe extern "system" fn custom_wndproc(
                    hwnd: HWND,
                    msg: u32,
                    wparam: WPARAM,
                    lparam: LPARAM,
                ) -> LRESULT {
                    // Get the original window procedure for this window
                    let original = {
                        let map = ORIGINAL_WNDPROCS.lock().unwrap();
                        map.as_ref().and_then(|m| m.get(&(hwnd as isize)).copied())
                    };

                    // Handle hit test messages to enable resize
                    if msg == WM_NCHITTEST {
                        unsafe {
                            // Call original window procedure
                            let default_result = if let Some(orig) = original {
                                CallWindowProcW(
                                    Some(std::mem::transmute(orig)),
                                    hwnd,
                                    msg,
                                    wparam,
                                    lparam,
                                )
                            } else {
                                HTCLIENT as isize
                            };

                            // Only override if in client area
                            if default_result == HTCLIENT as isize {
                                // Extract mouse coordinates from lparam
                                let x = (lparam & 0xFFFF) as i16 as i32;
                                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                                // Get window rectangle
                                let mut rect = std::mem::zeroed();
                                GetWindowRect(hwnd, &mut rect);

                                let border_width = 8;

                                // Check if mouse is in resize zones
                                let in_left = x - rect.left < border_width;
                                let in_right = rect.right - x < border_width;
                                let in_top = y - rect.top < border_width;
                                let in_bottom = rect.bottom - y < border_width;

                                // Return appropriate hit test value
                                // Corners have priority
                                if in_left && in_top {
                                    return HTTOPLEFT as isize;
                                } else if in_right && in_top {
                                    return HTTOPRIGHT as isize;
                                } else if in_left && in_bottom {
                                    return HTBOTTOMLEFT as isize;
                                } else if in_right && in_bottom {
                                    return HTBOTTOMRIGHT as isize;
                                }
                                // Then edges
                                else if in_left {
                                    return HTLEFT as isize;
                                } else if in_right {
                                    return HTRIGHT as isize;
                                } else if in_top {
                                    return HTTOP as isize;
                                } else if in_bottom {
                                    return HTBOTTOM as isize;
                                }
                            }

                            return default_result;
                        }
                    }

                    // For all other messages, call the original window procedure
                    if let Some(orig) = original {
                        unsafe {
                            CallWindowProcW(
                                Some(std::mem::transmute(orig)),
                                hwnd,
                                msg,
                                wparam,
                                lparam,
                            )
                        }
                    } else {
                        0
                    }
                }

                // Replace window procedure with our custom one
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, custom_wndproc as *const c_void as isize);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn setup_borderless_resize<W: HasWindowHandle>(_window: &W) {
    // No-op on non-Windows platforms
}
