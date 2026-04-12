unsafe fn log_d3d9_provider_info() {
    let d3d9_module = GetModuleHandleA(b"d3d9.dll\0".as_ptr());
    if d3d9_module.is_null() {
        log_warn("overlay", "D3D9 provider info unavailable: d3d9.dll not loaded");
        return;
    }

    let mut path_buf = [0u8; 512];
    let written = GetModuleFileNameA(d3d9_module, path_buf.as_mut_ptr(), path_buf.len() as u32);

    if written == 0 {
        log_warn("overlay", "D3D9 provider loaded but module path lookup failed");
        return;
    }

    let path = String::from_utf8_lossy(&path_buf[..written as usize]).to_string();
    let path_lower = path.to_ascii_lowercase();

    let provider_hint = if path_lower.contains("dxvk") {
        "dxvk"
    } else if path_lower.contains("dgvoodoo") {
        "dgvoodoo"
    } else if path_lower.contains("\\system32\\") {
        "native/system"
    } else {
        "unknown-wrapper-or-local"
    };

    log_info(
        "overlay",
        &format!(
            "D3D9 provider module: path='{}', hint={provider_hint}",
            path
        ),
    );
}

unsafe fn try_open_project_github() -> bool {
    type ShellExecuteAFn = unsafe extern "system" fn(
        hwnd: HWND,
        lpoperation: *const u8,
        lpfile: *const u8,
        lpparameters: *const u8,
        lpdirectory: *const u8,
        nshowcmd: i32,
    ) -> *mut c_void;

    let shell32_module = LoadLibraryA(b"shell32.dll\0".as_ptr());
    if shell32_module.is_null() {
        log_warn("overlay", "LoadLibraryA(shell32.dll) failed; cannot open GitHub URL");
        return false;
    }

    let shell_execute_addr = match resolve_proc_address(shell32_module, b"ShellExecuteA\0") {
        Some(addr) => addr,
        None => {
            log_warn(
                "overlay",
                "GetProcAddress(ShellExecuteA) failed; cannot open GitHub URL",
            );
            return false;
        }
    };

    let shell_execute: ShellExecuteAFn = core::mem::transmute(shell_execute_addr);
    let result = shell_execute(
        null_mut(),
        b"open\0".as_ptr(),
        b"https://github.com/hkAlice/tdu2-runtime-patch\0".as_ptr(),
        null(),
        null(),
        1,
    ) as isize;

    if result <= 32 {
        log_warn(
            "overlay",
            &format!("ShellExecuteA failed opening GitHub URL (code={result})"),
        );
        return false;
    }

    true
}

unsafe fn render_overlay_frame(
    state: &mut OverlayRenderState,
    device: *mut c_void,
    end_scene_calls: u32,
) -> Result<(), String> {
    let now = Instant::now();
    let mut delta = now.duration_since(state.last_frame).as_secs_f32();
    if !delta.is_finite() || delta <= 0.0 {
        delta = 1.0 / 60.0;
    }
    state.last_frame = now;

    let io = state.imgui.io_mut();
    io.delta_time = delta;
    if let Some(display_size) = query_display_size(device) {
        io.display_size = display_size;
    }

    let input_capture = OVERLAY_INPUT_CAPTURE_ENABLED.load(Ordering::Relaxed);
    io.mouse_draw_cursor = input_capture;
    if input_capture {
        update_imgui_mouse_position_from_system(io);
    }

    let runtime_state = crate::runtime_patches::runtime_patch_panel_state();
    let anti_tamper_enabled = runtime_state.anti_tamper_enabled;
    let mut dlc_fix_enabled = runtime_state.dlc_fix_enabled;
    let mut camera_fix_enabled = runtime_state.camera_fix_enabled;
    let mut camera_shake_fix_enabled = runtime_state.camera_shake_fix_enabled;
    let mut fov_enabled = runtime_state.fov_enabled;
    let mut fov_multiplier = runtime_state.fov_multiplier;
    let panel_visible = OVERLAY_PANEL_VISIBLE.load(Ordering::Relaxed);
    let input_capture = OVERLAY_INPUT_CAPTURE_ENABLED.load(Ordering::Relaxed);

    let ui = state.imgui.frame();
    ui.window("tdu2-runtime-patch")
        .size([560.0, 360.0], Condition::FirstUseEver)
        .position([20.0, 20.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
            ui.text(format!(
                "Commit: {}",
                option_env!("GIT_COMMIT_HASH").unwrap_or("unknown")
            ));
            if ui.button("Open GitHub") && !unsafe { try_open_project_github() } {
                log_warn("overlay", "GitHub button click could not open URL");
            }
            ui.text("Runtime controls");
            ui.separator();
            let mut persist_requested = false;

            ui.text(format!(
                "AntiTamper: {} (required, always ON)",
                enabled_label(anti_tamper_enabled)
            ));
            ui.text("  Prevents SecuROM VM/runtime checks from killing patched/hooked execution.");

            if ui.checkbox("DlcCarDealerFix", &mut dlc_fix_enabled) {
                dlc_fix_enabled = crate::runtime_patches::set_runtime_dlc_fix_enabled(dlc_fix_enabled);
                persist_requested = true;
            }
            ui.text("  Enables offline purchase of DLC cars in dealerships.");

            if ui.checkbox("CameraFix", &mut camera_fix_enabled) {
                camera_fix_enabled =
                    crate::runtime_patches::set_runtime_camera_fix_enabled(camera_fix_enabled);
                persist_requested = true;
            }
            ui.text("  Applies camera movement/frame-time stability patches.");

            if ui.checkbox("CameraShakeFix", &mut camera_shake_fix_enabled) {
                camera_shake_fix_enabled =
                    crate::runtime_patches::set_runtime_camera_shake_fix_enabled(
                        camera_shake_fix_enabled,
                    );
                persist_requested = true;
            }
            ui.text("  Suppresses exterior camera shake/jitter accumulators.");

            if ui.checkbox("FOV Enabled", &mut fov_enabled) {
                fov_enabled = crate::runtime_patches::set_runtime_fov_enabled(fov_enabled);
                persist_requested = true;
            }
            ui.text("  Enables/disables the FOV multiplier runtime hook.");

            if ui.slider("FOV Multiplier", 0.1, 4.0, &mut fov_multiplier) {
                fov_multiplier = crate::runtime_patches::set_runtime_fov_multiplier(fov_multiplier);
                persist_requested = true;
            }

            if persist_requested && !crate::runtime_patches::persist_runtime_panel_options() {
                log_warn("overlay", "Failed to persist runtime panel options to config");
            }

            ui.text(format!("FOV Multiplier: {:.3}", fov_multiplier));
            ui.text(format!(
                "PanelVisible: {} (toggle: F8)",
                enabled_label(panel_visible)
            ));
            ui.text(format!("InputCapture: {}", enabled_label(input_capture)));
            ui.separator();
            ui.text(format!("Present calls: {end_scene_calls}"));
        });

    let draw_data = state.imgui.render();
    state
        .renderer
        .render(draw_data)
        .map_err(|err| format!("imgui render failed: {err:?}"))
}

unsafe fn render_overlay_frame_for_present(
    state: &mut OverlayRenderState,
    device: *mut c_void,
    present_calls: u32,
) -> Result<(), String> {
    if device.is_null() {
        return Err(String::from("null IDirect3DDevice9 pointer"));
    }

    add_ref_com_object(device);
    let device_iface = IDirect3DDevice9::from_raw(device as _);

    let mut original_rt: Option<IDirect3DSurface9> = None;
    let mut backbuffer_bound = false;

    let bind_result = (|| -> Result<(), String> {
        let current_rt = device_iface
            .GetRenderTarget(0)
            .map_err(|err| format!("GetRenderTarget(0) failed: {err:?}"))?;
        original_rt = Some(current_rt);

        let back_buffer_surface = device_iface
            .GetBackBuffer(0, 0, D3DBACKBUFFER_TYPE_MONO)
            .map_err(|err| format!("GetBackBuffer(0,0) failed: {err:?}"))?;

        device_iface
            .SetRenderTarget(0, &back_buffer_surface)
            .map_err(|err| format!("SetRenderTarget(backbuffer) failed: {err:?}"))?;

        backbuffer_bound = true;

        if !OVERLAY_BACKBUFFER_BIND_LOGGED.swap(true, Ordering::Relaxed) {
            log_info("overlay", "Overlay draw target bound to swapchain backbuffer");
        }

        Ok(())
    })();

    if let Err(err) = bind_result {
        if present_calls <= 5 || present_calls % 600 == 0 {
            log_warn(
                "overlay",
                &format!("Backbuffer bind skipped, drawing on current target instead: {err}"),
            );
        }
    }

    let render_result = render_overlay_frame(state, device, present_calls);

    if backbuffer_bound {
        if let Some(surface) = original_rt.as_ref() {
            if let Err(err) = device_iface.SetRenderTarget(0, surface) {
                if present_calls <= 5 || present_calls % 600 == 0 {
                    log_warn(
                        "overlay",
                        &format!("Failed to restore original render target: {err:?}"),
                    );
                }
            }
        }
    }

    render_result
}

pub(crate) fn set_overlay_panel_feature_state(
    anti_tamper_enabled: bool,
    dlc_fix_enabled: bool,
    camera_fix_enabled: bool,
    camera_shake_fix_enabled: bool,
    fov_enabled: bool,
    fov_multiplier: f32,
) {
    PANEL_ANTI_TAMPER_ENABLED.store(anti_tamper_enabled, Ordering::Relaxed);
    PANEL_DLC_FIX_ENABLED.store(dlc_fix_enabled, Ordering::Relaxed);
    PANEL_CAMERA_FIX_ENABLED.store(camera_fix_enabled, Ordering::Relaxed);
    PANEL_CAMERA_SHAKE_FIX_ENABLED.store(camera_shake_fix_enabled, Ordering::Relaxed);
    PANEL_FOV_ENABLED.store(fov_enabled, Ordering::Relaxed);
    PANEL_FOV_BITS.store(fov_multiplier.to_bits(), Ordering::Relaxed);
}

unsafe extern "system" fn hook_present(
    device: *mut c_void,
    src_rect: *const c_void,
    dst_rect: *const c_void,
    dst_window_override: HWND,
    dirty_region: *const c_void,
) -> i32 {
    let call_count = PRESENT_CALL_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if call_count <= 5 || call_count % 600 == 0 {
        log_info("overlay", &format!("D3D9 Present tick: calls={call_count}"));
    }

    let orig = ORIG_PRESENT.load(Ordering::Relaxed);
    if orig == 0 {
        return 0;
    }

    let orig_fn: PresentFn = core::mem::transmute(orig);

    try_install_overlay_wndproc_hook(device, dst_window_override);

    if !OVERLAY_PANEL_VISIBLE.load(Ordering::Relaxed) {
        return orig_fn(device, src_rect, dst_rect, dst_window_override, dirty_region);
    }

    let retry_after_call = OVERLAY_RETRY_AFTER_CALL.load(Ordering::Relaxed);
    if call_count >= retry_after_call {
        let begin_hr = begin_scene(device);
        if begin_hr < 0 {
            if call_count <= 5 || call_count % 600 == 0 {
                log_warn(
                    "overlay",
                    &format!("Skipping ImGui render: BeginScene failed hr={:#x}", begin_hr as u32),
                );
            }
            OVERLAY_RETRY_AFTER_CALL.store(call_count.saturating_add(300), Ordering::Relaxed);
        } else {
            OVERLAY_RENDER_STATE.with(|slot| {
                let mut overlay_state = slot.borrow_mut();

                if overlay_state.is_none() {
                    match unsafe { initialize_imgui_overlay(device) } {
                        Ok(state) => {
                            log_info("overlay", "ImGui read-only panel initialized");
                            *overlay_state = Some(state);
                        }
                        Err(err) => {
                            if call_count <= 5 || call_count % 600 == 0 {
                                log_warn("overlay", &format!("ImGui init skipped: {err}"));
                            }
                        }
                    }
                }

                if let Some(state) = overlay_state.as_mut() {
                    match unsafe { render_overlay_frame_for_present(state, device, call_count) } {
                        Ok(()) => {
                            if !OVERLAY_FIRST_SUCCESSFUL_RENDER_LOGGED.swap(true, Ordering::Relaxed)
                            {
                                let display_size = state.imgui.io().display_size;
                                log_info(
                                    "overlay",
                                    &format!(
                                        "ImGui panel rendered first frame (display={:.0}x{:.0})",
                                        display_size[0], display_size[1]
                                    ),
                                );
                            }
                        }
                        Err(err) => {
                            log_warn(
                                "overlay",
                                &format!("ImGui panel render failed, dropping state: {err}"),
                            );
                            *overlay_state = None;
                            OVERLAY_RETRY_AFTER_CALL
                                .store(call_count.saturating_add(300), Ordering::Relaxed);
                        }
                    }
                }
            });

            let end_hr = end_scene(device);
            if end_hr < 0 && (call_count <= 5 || call_count % 600 == 0) {
                log_warn(
                    "overlay",
                    &format!("Overlay EndScene failed hr={:#x}", end_hr as u32),
                );
            }
        }
    }

    orig_fn(device, src_rect, dst_rect, dst_window_override, dirty_region)
}

unsafe extern "system" fn hook_reset(
    device: *mut c_void,
    params: *mut D3dPresentParameters,
) -> i32 {
    log_info("overlay", "D3D9 Reset detected");

    OVERLAY_RENDER_STATE.with(|slot| {
        let had_state = slot.borrow().is_some();
        if had_state {
            log_info(
                "overlay",
                "Dropping ImGui panel renderer state before device Reset",
            );
        }
        *slot.borrow_mut() = None;
    });

    let orig = ORIG_RESET.load(Ordering::Relaxed);
    if orig == 0 {
        return 0;
    }

    let orig_fn: ResetFn = core::mem::transmute(orig);
    orig_fn(device, params)
}

pub(crate) unsafe fn install_d3d9_overlay_hooks() -> bool {
    if D3D9_HOOKS_INSTALLED.load(Ordering::Relaxed) {
        return true;
    }

    let device = match create_dummy_device() {
        Some(device) => device,
        None => return false,
    };

    let vtable = get_vtable(device);
    if vtable.is_null() {
        log_error("overlay", "IDirect3DDevice9 vtable pointer is null");
        return false;
    }

    let release_device: D3D9ReleaseFn =
        core::mem::transmute(*vtable.add(IDIRECT3DDEVICE9_VTBL_RELEASE_INDEX));

    let present_detour = hook_present as *const () as usize;
    let present_original = match patch_vtable_entry(
        vtable,
        IDIRECT3DDEVICE9_VTBL_PRESENT_INDEX,
        present_detour,
        "D3D9 Present vtable",
    ) {
        Some(original) => original,
        None => {
            release_device(device);
            return false;
        }
    };
    ORIG_PRESENT.store(present_original, Ordering::Relaxed);

    let reset_detour = hook_reset as *const () as usize;
    if let Some(reset_original) = patch_vtable_entry(
        vtable,
        IDIRECT3DDEVICE9_VTBL_RESET_INDEX,
        reset_detour,
        "D3D9 Reset vtable",
    ) {
        ORIG_RESET.store(reset_original, Ordering::Relaxed);
    } else {
        log_warn(
            "overlay",
            "D3D9 Reset vtable hook not installed; continuing with EndScene heartbeat only",
        );
    }

    release_device(device);

    D3D9_HOOKS_INSTALLED.store(true, Ordering::Relaxed);

    if !install_dinput_mouse_suppression_hooks() {
        log_warn("overlay", "DirectInput mouse suppression hook bootstrap failed");
    }

    log_d3d9_provider_info();

    log_info(
        "overlay",
        &format!(
            "Installed D3D9 overlay hooks via vtable patch: vtable={:#x}, Present={:#x}, Reset={:#x}",
            vtable as usize,
            ORIG_PRESENT.load(Ordering::Relaxed),
            ORIG_RESET.load(Ordering::Relaxed)
        ),
    );

    true
}
