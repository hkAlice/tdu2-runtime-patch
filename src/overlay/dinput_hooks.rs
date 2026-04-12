#[inline]
fn is_directinput_mouse_offset(dw_ofs: u32) -> bool {
    matches!(dw_ofs, 0 | 4 | 8 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19)
}

unsafe extern "system" fn hook_dinput_get_device_state(
    this: *mut c_void,
    cb_data: u32,
    data: *mut c_void,
) -> i32 {
    let orig = ORIG_DINPUT_GET_DEVICE_STATE.load(Ordering::Relaxed);
    if orig == 0 {
        return -1;
    }

    let orig_fn: DiGetDeviceStateFn = core::mem::transmute(orig);
    let hr = orig_fn(this, cb_data, data);

    if hr >= 0 && OVERLAY_INPUT_CAPTURE_ENABLED.load(Ordering::Relaxed) && !data.is_null() {
        let cb_data = cb_data as usize;
        if cb_data == core::mem::size_of::<DIMOUSESTATE>()
            || cb_data == core::mem::size_of::<DIMOUSESTATE2>()
        {
            std::ptr::write_bytes(data as *mut u8, 0, cb_data);
        }
    }

    hr
}

unsafe extern "system" fn hook_dinput_get_device_data(
    this: *mut c_void,
    cb_object_data: u32,
    rgdod: *mut DIDEVICEOBJECTDATA,
    pdw_inout: *mut u32,
    flags: u32,
) -> i32 {
    let orig = ORIG_DINPUT_GET_DEVICE_DATA.load(Ordering::Relaxed);
    if orig == 0 {
        return -1;
    }

    let orig_fn: DiGetDeviceDataFn = core::mem::transmute(orig);
    let hr = orig_fn(this, cb_object_data, rgdod, pdw_inout, flags);

    if hr < 0 || !OVERLAY_INPUT_CAPTURE_ENABLED.load(Ordering::Relaxed) {
        return hr;
    }

    if pdw_inout.is_null() {
        return hr;
    }

    if rgdod.is_null() {
        *pdw_inout = 0;
        return hr;
    }

    if cb_object_data as usize != core::mem::size_of::<DIDEVICEOBJECTDATA>() {
        return hr;
    }

    let count = *pdw_inout as usize;
    if count == 0 {
        return hr;
    }

    let events = core::slice::from_raw_parts_mut(rgdod, count);
    let looks_like_mouse = events.iter().all(|event| is_directinput_mouse_offset(event.dwOfs));

    if looks_like_mouse {
        for event in events.iter_mut() {
            event.dwData = 0;
        }
        *pdw_inout = 0;
    }

    hr
}

unsafe fn install_dinput_mouse_suppression_hooks() -> bool {
    if DINPUT_HOOKS_INSTALLED.load(Ordering::Relaxed) {
        return true;
    }

    let dinput8_module = LoadLibraryA(b"dinput8.dll\0".as_ptr());
    if dinput8_module.is_null() {
        log_warn("overlay", "LoadLibraryA(dinput8.dll) failed; mouse suppression disabled");
        return false;
    }

    let create_addr = match resolve_proc_address(dinput8_module, b"DirectInput8Create\0") {
        Some(addr) => addr,
        None => {
            log_warn(
                "overlay",
                "GetProcAddress(DirectInput8Create) failed; mouse suppression disabled",
            );
            return false;
        }
    };

    let create_fn: DirectInput8CreateFn = core::mem::transmute(create_addr);
    let mut dinput_raw: *mut c_void = null_mut();
    let hinstance = GetModuleHandleA(null());

    let create_hr = create_fn(
        hinstance as *mut c_void,
        DIRECTINPUT_VERSION,
        &IDirectInput8A::IID,
        &mut dinput_raw,
        null_mut(),
    );

    if create_hr < 0 || dinput_raw.is_null() {
        log_warn(
            "overlay",
            &format!(
                "DirectInput8Create failed (hr={:#x}); mouse suppression disabled",
                create_hr as u32
            ),
        );
        return false;
    }

    let dinput_iface = IDirectInput8A::from_raw(dinput_raw as _);
    let mut mouse_device: Option<IDirectInputDevice8A> = None;

    if let Err(err) =
        dinput_iface.CreateDevice(&GUID_SysMouse, &mut mouse_device, None::<&IUnknown>)
    {
        log_warn(
            "overlay",
            &format!(
                "IDirectInput8A::CreateDevice(GUID_SysMouse) failed; mouse suppression disabled: {err:?}"
            ),
        );
        drop(dinput_iface);
        return false;
    }

    let Some(mouse_device_iface) = mouse_device.as_ref() else {
        log_warn(
            "overlay",
            "CreateDevice(GUID_SysMouse) returned null; mouse suppression disabled",
        );
        drop(dinput_iface);
        return false;
    };

    let mouse_raw = Vtable::as_raw(mouse_device_iface) as *mut c_void;
    let mouse_vtable = get_vtable(mouse_raw);
    if mouse_vtable.is_null() {
        log_warn(
            "overlay",
            "DirectInput mouse vtable is null; mouse suppression disabled",
        );
        drop(mouse_device);
        drop(dinput_iface);
        return false;
    }

    let get_state_detour = hook_dinput_get_device_state as *const () as usize;
    let Some(get_state_original) = patch_vtable_entry(
        mouse_vtable,
        IDIRECTINPUTDEVICE8_VTBL_GETDEVICESTATE_INDEX,
        get_state_detour,
        "DirectInput GetDeviceState vtable",
    ) else {
        drop(mouse_device);
        drop(dinput_iface);
        return false;
    };

    ORIG_DINPUT_GET_DEVICE_STATE.store(get_state_original, Ordering::Relaxed);

    let get_data_detour = hook_dinput_get_device_data as *const () as usize;
    if let Some(get_data_original) = patch_vtable_entry(
        mouse_vtable,
        IDIRECTINPUTDEVICE8_VTBL_GETDEVICEDATA_INDEX,
        get_data_detour,
        "DirectInput GetDeviceData vtable",
    ) {
        ORIG_DINPUT_GET_DEVICE_DATA.store(get_data_original, Ordering::Relaxed);
    } else {
        log_warn(
            "overlay",
            "DirectInput GetDeviceData hook not installed; GetDeviceState suppression still active",
        );
    }

    drop(mouse_device);
    drop(dinput_iface);

    DINPUT_HOOKS_INSTALLED.store(true, Ordering::Relaxed);
    log_info(
        "overlay",
        "Installed DirectInput mouse suppression hooks (active when overlay input capture is ON)",
    );

    true
}
