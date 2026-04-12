unsafe fn process_overlay_toggle_hotkey_message(msg: u32, wparam: WPARAM) -> bool {
    if wparam != OVERLAY_TOGGLE_KEY_VK_F8 {
        return false;
    }

    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => true,
        WM_KEYUP | WM_SYSKEYUP => {
            let panel_visible = !OVERLAY_PANEL_VISIBLE.load(Ordering::Relaxed);
            OVERLAY_PANEL_VISIBLE.store(panel_visible, Ordering::Relaxed);
            OVERLAY_INPUT_CAPTURE_ENABLED.store(panel_visible, Ordering::Relaxed);
            set_overlay_cursor_capture(panel_visible);

            log_info(
                "overlay",
                &format!(
                    "Overlay toggled via F8: visible={}, input_capture={}",
                    enabled_label(panel_visible),
                    enabled_label(panel_visible)
                ),
            );

            true
        }
        _ => false,
    }
}

unsafe fn call_original_overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original_bits = OVERLAY_ORIGINAL_WNDPROC_BITS.load(Ordering::Relaxed);
    if original_bits == 0 {
        return DefWindowProcA(hwnd, msg, wparam, lparam);
    }

    let original_addr = bits_to_i32(original_bits) as isize;
    let original_proc: WNDPROC = Some(core::mem::transmute(original_addr));
    CallWindowProcA(original_proc, hwnd, msg, wparam, lparam)
}

unsafe fn remove_overlay_wndproc_hook(reason: &str) {
    if !OVERLAY_WNDPROC_HOOK_INSTALLED.swap(false, Ordering::Relaxed) {
        return;
    }

    let hwnd_bits = OVERLAY_WNDPROC_HWND_BITS.swap(0, Ordering::Relaxed);
    let original_bits = OVERLAY_ORIGINAL_WNDPROC_BITS.swap(0, Ordering::Relaxed);
    let hwnd = bits_to_hwnd(hwnd_bits);

    if !hwnd.is_null() && original_bits != 0 {
        let _ = SetWindowLongA(hwnd, GWL_WNDPROC, bits_to_i32(original_bits));
    }

    OVERLAY_PANEL_VISIBLE.store(false, Ordering::Relaxed);
    OVERLAY_INPUT_CAPTURE_ENABLED.store(false, Ordering::Relaxed);
    set_overlay_cursor_capture(false);

    log_info("overlay", &format!("Removed overlay input hook ({reason})"));
}

unsafe fn install_overlay_wndproc_hook(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }

    let overlay_proc = (overlay_input_wndproc as *const () as usize as u32) as i32;
    let previous_proc = SetWindowLongA(hwnd, GWL_WNDPROC, overlay_proc);
    if previous_proc == 0 {
        if !OVERLAY_WNDPROC_INSTALL_FAILED_LOGGED.swap(true, Ordering::Relaxed) {
            log_warn(
                "overlay",
                &format!(
                    "SetWindowLongA(GWL_WNDPROC) failed for overlay input hook (hwnd={:#x})",
                    hwnd as usize
                ),
            );
        }
        return false;
    }

    OVERLAY_WNDPROC_INSTALL_FAILED_LOGGED.store(false, Ordering::Relaxed);
    OVERLAY_ORIGINAL_WNDPROC_BITS.store(i32_to_bits(previous_proc), Ordering::Relaxed);
    OVERLAY_WNDPROC_HWND_BITS.store(hwnd_to_bits(hwnd), Ordering::Relaxed);
    OVERLAY_WNDPROC_HOOK_INSTALLED.store(true, Ordering::Relaxed);

    log_info(
        "overlay",
        &format!(
            "Installed overlay input hook on hwnd={:#x} (F8 toggles panel/input capture)",
            hwnd as usize
        ),
    );

    true
}

unsafe fn resolve_overlay_target_hwnd(
    device: *mut c_void,
    dst_window_override: HWND,
) -> Option<HWND> {
    if !dst_window_override.is_null() {
        return Some(dst_window_override);
    }

    let cached_hwnd = bits_to_hwnd(OVERLAY_TARGET_HWND_BITS.load(Ordering::Relaxed));
    if !cached_hwnd.is_null() {
        return Some(cached_hwnd);
    }

    if device.is_null() {
        return None;
    }

    add_ref_com_object(device);
    let device_iface = IDirect3DDevice9::from_raw(device as _);
    let mut creation_params = D3DDEVICE_CREATION_PARAMETERS::default();
    let result = device_iface.GetCreationParameters(&mut creation_params);

    drop(device_iface);

    if result.is_err() {
        return None;
    }

    let focus_window = creation_params.hFocusWindow.0 as HWND;
    if focus_window.is_null() {
        None
    } else {
        Some(focus_window)
    }
}

unsafe fn try_install_overlay_wndproc_hook(device: *mut c_void, dst_window_override: HWND) {
    let Some(target_hwnd) = resolve_overlay_target_hwnd(device, dst_window_override) else {
        return;
    };

    OVERLAY_TARGET_HWND_BITS.store(hwnd_to_bits(target_hwnd), Ordering::Relaxed);

    let current_hwnd = bits_to_hwnd(OVERLAY_WNDPROC_HWND_BITS.load(Ordering::Relaxed));
    let hook_installed = OVERLAY_WNDPROC_HOOK_INSTALLED.load(Ordering::Relaxed);

    if hook_installed && current_hwnd == target_hwnd {
        return;
    }

    if hook_installed && !current_hwnd.is_null() && current_hwnd != target_hwnd {
        remove_overlay_wndproc_hook("switching target window");
    }

    let _ = install_overlay_wndproc_hook(target_hwnd);
}

unsafe extern "system" fn overlay_input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if process_overlay_toggle_hotkey_message(msg, wparam) {
        return 0;
    }

    if msg == WM_NCDESTROY {
        let result = call_original_overlay_wndproc(hwnd, msg, wparam, lparam);
        remove_overlay_wndproc_hook("WM_NCDESTROY");
        OVERLAY_TARGET_HWND_BITS.store(0, Ordering::Relaxed);
        return result;
    }

    if OVERLAY_INPUT_CAPTURE_ENABLED.load(Ordering::Relaxed) {
        let handled = OVERLAY_RENDER_STATE.with(|slot| {
            let mut render_state_slot = slot.borrow_mut();
            if let Some(state) = render_state_slot.as_mut() {
                handle_overlay_input_message(state, msg, wparam, lparam)
            } else {
                is_overlay_input_message(msg)
            }
        });

        if handled {
            return 0;
        }
    }

    call_original_overlay_wndproc(hwnd, msg, wparam, lparam)
}
