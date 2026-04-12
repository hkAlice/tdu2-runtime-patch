unsafe fn resolve_proc_address(module: *mut c_void, symbol: &'static [u8]) -> Option<usize> {
    GetProcAddress(module, symbol.as_ptr()).map(|proc| proc as usize)
}

unsafe fn get_vtable(instance: *mut c_void) -> *mut usize {
    *(instance as *mut *mut usize)
}

unsafe fn add_ref_com_object(instance: *mut c_void) {
    if instance.is_null() {
        return;
    }

    let vtable = get_vtable(instance);
    if vtable.is_null() {
        return;
    }

    let add_ref_addr = *vtable.add(1);
    let add_ref: unsafe extern "system" fn(*mut c_void) -> u32 =
        core::mem::transmute(add_ref_addr);
    let _ = add_ref(instance);
}

unsafe fn initialize_imgui_overlay(device: *mut c_void) -> Result<OverlayRenderState, String> {
    if device.is_null() {
        return Err(String::from("null IDirect3DDevice9 pointer"));
    }

    let mut imgui = imgui::Context::create();
    imgui.set_ini_filename(None);
    imgui.io_mut().display_size = query_display_size(device).unwrap_or([1280.0, 720.0]);
    imgui.io_mut().delta_time = 1.0 / 60.0;

    add_ref_com_object(device);
    let device_iface = IDirect3DDevice9::from_raw(device as _);

    let renderer = Renderer::new_raw(&mut imgui, device_iface)
        .map_err(|err| format!("imgui renderer init failed: {err:?}"))?;

    Ok(OverlayRenderState {
        imgui,
        renderer,
        last_frame: Instant::now(),
    })
}

fn enabled_label(enabled: bool) -> &'static str {
    if enabled {
        "ON"
    } else {
        "OFF"
    }
}

unsafe fn set_overlay_cursor_capture(enabled: bool) {
    if enabled {
        if OVERLAY_CURSOR_CAPTURE_ACTIVE.swap(true, Ordering::Relaxed) {
            return;
        }

        let _ = ClipCursor(null());

        let mut show_cursor_delta = 0;
        for _ in 0..8 {
            show_cursor_delta += 1;
            if ShowCursor(1) >= 0 {
                break;
            }
        }

        OVERLAY_SHOWCURSOR_DELTA.store(show_cursor_delta, Ordering::Relaxed);

        let cursor = LoadCursorW(null_mut(), IDC_ARROW);
        if !cursor.is_null() {
            let _ = SetCursor(cursor);
        }

        log_info("overlay", "Overlay cursor capture enabled");
    } else {
        if !OVERLAY_CURSOR_CAPTURE_ACTIVE.swap(false, Ordering::Relaxed) {
            return;
        }

        let show_cursor_delta = OVERLAY_SHOWCURSOR_DELTA.swap(0, Ordering::Relaxed).max(0);
        for _ in 0..show_cursor_delta {
            let _ = ShowCursor(0);
        }

        log_info("overlay", "Overlay cursor capture disabled");
    }
}

unsafe fn update_imgui_mouse_position_from_system(io: &mut imgui::Io) {
    let hwnd = bits_to_hwnd(OVERLAY_WNDPROC_HWND_BITS.load(Ordering::Relaxed));
    if hwnd.is_null() {
        return;
    }

    let mut cursor_pos = POINT { x: 0, y: 0 };
    if GetCursorPos(&mut cursor_pos) == 0 {
        return;
    }

    let mut window_rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    if GetWindowRect(hwnd, &mut window_rect) == 0 {
        return;
    }

    io.add_mouse_pos_event([
        (cursor_pos.x - window_rect.left) as f32,
        (cursor_pos.y - window_rect.top) as f32,
    ]);
}

#[inline]
fn hwnd_to_bits(hwnd: HWND) -> usize {
    hwnd as usize
}

#[inline]
fn bits_to_hwnd(bits: usize) -> HWND {
    bits as HWND
}

#[inline]
fn i32_to_bits(value: i32) -> usize {
    value as u32 as usize
}

#[inline]
fn bits_to_i32(bits: usize) -> i32 {
    bits as u32 as i32
}

#[inline]
fn lparam_low_word_signed(lparam: LPARAM) -> i16 {
    (lparam as u32 & 0xFFFF) as u16 as i16
}

#[inline]
fn lparam_high_word_signed(lparam: LPARAM) -> i16 {
    ((lparam as u32 >> 16) & 0xFFFF) as u16 as i16
}

#[inline]
fn wparam_high_word_signed(wparam: WPARAM) -> i16 {
    ((wparam as u32 >> 16) & 0xFFFF) as u16 as i16
}

fn map_alpha_key(vk: u32) -> Option<Key> {
    const ALPHA_KEYS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];

    if (VK_A..=VK_Z).contains(&vk) {
        Some(ALPHA_KEYS[(vk - VK_A) as usize])
    } else {
        None
    }
}

fn map_number_key(vk: u32) -> Option<Key> {
    const NUMBER_KEYS: [Key; 10] = [
        Key::Alpha0,
        Key::Alpha1,
        Key::Alpha2,
        Key::Alpha3,
        Key::Alpha4,
        Key::Alpha5,
        Key::Alpha6,
        Key::Alpha7,
        Key::Alpha8,
        Key::Alpha9,
    ];

    if (VK_ALPHA_0..=VK_ALPHA_9).contains(&vk) {
        Some(NUMBER_KEYS[(vk - VK_ALPHA_0) as usize])
    } else {
        None
    }
}

fn map_function_key(vk: u32) -> Option<Key> {
    const FUNCTION_KEYS: [Key; 12] = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];

    if (VK_F1..=VK_F12).contains(&vk) {
        Some(FUNCTION_KEYS[(vk - VK_F1) as usize])
    } else {
        None
    }
}

fn map_virtual_key_to_imgui_key(vk: u32, lparam: LPARAM) -> Option<Key> {
    let is_extended = (lparam as u32 & 0x0100_0000) != 0;

    match vk {
        VK_TAB => Some(Key::Tab),
        VK_LEFT => Some(Key::LeftArrow),
        VK_RIGHT => Some(Key::RightArrow),
        VK_UP => Some(Key::UpArrow),
        VK_DOWN => Some(Key::DownArrow),
        VK_PRIOR => Some(Key::PageUp),
        VK_NEXT => Some(Key::PageDown),
        VK_HOME => Some(Key::Home),
        VK_END => Some(Key::End),
        VK_INSERT => Some(Key::Insert),
        VK_DELETE => Some(Key::Delete),
        VK_BACK => Some(Key::Backspace),
        VK_SPACE => Some(Key::Space),
        VK_RETURN => {
            if is_extended {
                Some(Key::KeypadEnter)
            } else {
                Some(Key::Enter)
            }
        }
        VK_ESCAPE => Some(Key::Escape),
        VK_SHIFT => Some(Key::LeftShift),
        VK_CONTROL => {
            if is_extended {
                Some(Key::RightCtrl)
            } else {
                Some(Key::LeftCtrl)
            }
        }
        VK_MENU => {
            if is_extended {
                Some(Key::RightAlt)
            } else {
                Some(Key::LeftAlt)
            }
        }
        VK_LSHIFT => Some(Key::LeftShift),
        VK_RSHIFT => Some(Key::RightShift),
        VK_LCONTROL => Some(Key::LeftCtrl),
        VK_RCONTROL => Some(Key::RightCtrl),
        VK_LMENU => Some(Key::LeftAlt),
        VK_RMENU => Some(Key::RightAlt),
        VK_LWIN => Some(Key::LeftSuper),
        VK_RWIN => Some(Key::RightSuper),
        _ => map_alpha_key(vk)
            .or_else(|| map_number_key(vk))
            .or_else(|| map_function_key(vk)),
    }
}

fn is_overlay_input_message(msg: u32) -> bool {
    matches!(
        msg,
        WM_INPUT
            | WM_SETCURSOR
            | WM_MOUSEMOVE
            | WM_LBUTTONDOWN
            | WM_LBUTTONUP
            | WM_RBUTTONDOWN
            | WM_RBUTTONUP
            | WM_MBUTTONDOWN
            | WM_MBUTTONUP
            | WM_XBUTTONDOWN
            | WM_XBUTTONUP
            | WM_MOUSEWHEEL
            | WM_MOUSEHWHEEL
            | WM_KEYDOWN
            | WM_KEYUP
            | WM_SYSKEYDOWN
            | WM_SYSKEYUP
            | WM_CHAR
    )
}

fn handle_overlay_input_message(
    state: &mut OverlayRenderState,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> bool {
    let io = state.imgui.io_mut();

    match msg {
        WM_INPUT => true,
        WM_SETCURSOR => {
            let cursor = unsafe { LoadCursorW(null_mut(), IDC_ARROW) };
            if !cursor.is_null() {
                let _ = unsafe { SetCursor(cursor) };
            }
            true
        }
        WM_MOUSEMOVE => {
            io.add_mouse_pos_event([
                lparam_low_word_signed(lparam) as f32,
                lparam_high_word_signed(lparam) as f32,
            ]);
            true
        }
        WM_LBUTTONDOWN => {
            io.add_mouse_button_event(MouseButton::Left, true);
            true
        }
        WM_LBUTTONUP => {
            io.add_mouse_button_event(MouseButton::Left, false);
            true
        }
        WM_RBUTTONDOWN => {
            io.add_mouse_button_event(MouseButton::Right, true);
            true
        }
        WM_RBUTTONUP => {
            io.add_mouse_button_event(MouseButton::Right, false);
            true
        }
        WM_MBUTTONDOWN => {
            io.add_mouse_button_event(MouseButton::Middle, true);
            true
        }
        WM_MBUTTONUP => {
            io.add_mouse_button_event(MouseButton::Middle, false);
            true
        }
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let xbutton = ((wparam as u32 >> 16) & 0xFFFF) as u16;
            let button = if xbutton == XBUTTON1 as u16 {
                MouseButton::Extra1
            } else {
                MouseButton::Extra2
            };
            io.add_mouse_button_event(button, msg == WM_XBUTTONDOWN);
            true
        }
        WM_MOUSEWHEEL => {
            io.add_mouse_wheel_event([0.0, wparam_high_word_signed(wparam) as f32 / 120.0]);
            true
        }
        WM_MOUSEHWHEEL => {
            io.add_mouse_wheel_event([wparam_high_word_signed(wparam) as f32 / 120.0, 0.0]);
            true
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
            let key_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let vk = wparam as u32;

            if let Some(key) = map_virtual_key_to_imgui_key(vk, lparam) {
                io.add_key_event(key, key_down);

                match key {
                    Key::LeftCtrl | Key::RightCtrl => io.add_key_event(Key::ModCtrl, key_down),
                    Key::LeftShift | Key::RightShift => {
                        io.add_key_event(Key::ModShift, key_down)
                    }
                    Key::LeftAlt | Key::RightAlt => io.add_key_event(Key::ModAlt, key_down),
                    Key::LeftSuper | Key::RightSuper => {
                        io.add_key_event(Key::ModSuper, key_down)
                    }
                    _ => {}
                }
            }

            true
        }
        WM_CHAR => {
            if let Some(ch) = char::from_u32(wparam as u32) {
                if !ch.is_control() {
                    io.add_input_character(ch);
                }
            }
            true
        }
        _ => false,
    }
}

unsafe fn query_display_size(device: *mut c_void) -> Option<[f32; 2]> {
    if device.is_null() {
        return None;
    }

    add_ref_com_object(device);
    let device_iface = IDirect3DDevice9::from_raw(device as _);

    let mut viewport = D3DVIEWPORT9::default();
    let viewport_result = device_iface.GetViewport(&mut viewport);

    drop(device_iface);

    if viewport_result.is_ok() && viewport.Width > 0 && viewport.Height > 0 {
        Some([viewport.Width as f32, viewport.Height as f32])
    } else {
        None
    }
}

unsafe fn begin_scene(device: *mut c_void) -> i32 {
    if device.is_null() {
        return i32::MIN;
    }

    let vtable = get_vtable(device);
    if vtable.is_null() {
        return i32::MIN;
    }

    let begin_scene_addr = *vtable.add(IDIRECT3DDEVICE9_VTBL_BEGINSCENE_INDEX);
    let begin_scene_fn: BeginSceneFn = core::mem::transmute(begin_scene_addr);
    begin_scene_fn(device)
}

unsafe fn end_scene(device: *mut c_void) -> i32 {
    if device.is_null() {
        return i32::MIN;
    }

    let vtable = get_vtable(device);
    if vtable.is_null() {
        return i32::MIN;
    }

    let end_scene_addr = *vtable.add(IDIRECT3DDEVICE9_VTBL_ENDSCENE_INDEX);
    let end_scene_fn: EndSceneFn = core::mem::transmute(end_scene_addr);
    end_scene_fn(device)
}

unsafe fn create_dummy_window() -> Option<HWND> {
    let hwnd = CreateWindowExA(
        0,
        b"STATIC\0".as_ptr(),
        b"tdu2-runtime-patch-d3d9\0".as_ptr(),
        0,
        0,
        0,
        16,
        16,
        null_mut(),
        null_mut(),
        null_mut(),
        null(),
    );

    if hwnd.is_null() {
        log_error("overlay", "CreateWindowExA failed for D3D9 probe window");
        None
    } else {
        Some(hwnd)
    }
}

unsafe fn create_dummy_device() -> Option<*mut c_void> {
    let d3d9_module = LoadLibraryA(b"d3d9.dll\0".as_ptr());
    if d3d9_module.is_null() {
        log_error("overlay", "LoadLibraryA(d3d9.dll) failed");
        return None;
    }

    let create9_addr = match resolve_proc_address(d3d9_module, b"Direct3DCreate9\0") {
        Some(addr) => addr,
        None => {
            log_error("overlay", "GetProcAddress(Direct3DCreate9) failed");
            return None;
        }
    };

    let create9: Direct3DCreate9Fn = core::mem::transmute(create9_addr);
    let d3d9 = create9(D3D_SDK_VERSION);
    if d3d9.is_null() {
        log_error("overlay", "Direct3DCreate9 returned null");
        return None;
    }

    let hwnd = match create_dummy_window() {
        Some(hwnd) => hwnd,
        None => {
            let d3d9_vtable = get_vtable(d3d9);
            let release_d3d9: D3D9ReleaseFn =
                core::mem::transmute(*d3d9_vtable.add(IDIRECT3D9_VTBL_RELEASE_INDEX));
            release_d3d9(d3d9);
            return None;
        }
    };

    let mut params: D3dPresentParameters = std::mem::zeroed();
    params.windowed = 1;
    params.swap_effect = D3DSWAPEFFECT_DISCARD;
    params.h_device_window = hwnd;

    let d3d9_vtable = get_vtable(d3d9);
    let create_device_addr = *d3d9_vtable.add(IDIRECT3D9_VTBL_CREATE_DEVICE_INDEX);
    let create_device: D3D9CreateDeviceFn = core::mem::transmute(create_device_addr);

    let mut device: *mut c_void = null_mut();
    let hr = create_device(
        d3d9,
        0,
        D3DDEVTYPE_HAL,
        hwnd,
        D3DCREATE_SOFTWARE_VERTEXPROCESSING,
        &mut params,
        &mut device,
    );

    let release_d3d9: D3D9ReleaseFn =
        core::mem::transmute(*d3d9_vtable.add(IDIRECT3D9_VTBL_RELEASE_INDEX));
    release_d3d9(d3d9);
    DestroyWindow(hwnd);

    if hr < 0 || device.is_null() {
        log_error(
            "overlay",
            &format!(
                "IDirect3D9::CreateDevice failed (hr={:#x})",
                hr as u32
            ),
        );
        return None;
    }

    Some(device)
}

unsafe fn patch_vtable_entry(
    vtable: *mut usize,
    index: usize,
    replacement: usize,
    tag: &str,
) -> Option<usize> {
    if vtable.is_null() {
        return None;
    }

    let entry = vtable.add(index);
    let original = *entry;
    if original == replacement {
        log_warn(
            "overlay",
            &format!(
                "{tag} already equals detour ({replacement:#x}); refusing to overwrite original pointer"
            ),
        );
        return None;
    }

    let replacement_bytes = (replacement as u32).to_le_bytes();
    patch_bytes(entry as usize, &replacement_bytes);

    let patched = *entry;
    if patched != replacement {
        log_error(
            "overlay",
            &format!(
                "{tag} patch verification failed: expected={replacement:#x}, got={patched:#x}"
            ),
        );
        return None;
    }

    log_info(
        "overlay",
        &format!(
            "{tag} patched: entry={:#x}, original={original:#x}, replacement={replacement:#x}",
            entry as usize
        ),
    );

    Some(original)
}
