use core::ffi::c_void;
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::time::Instant;

use imgui::{Condition, Key, MouseButton};
use imgui_dx9_renderer::Renderer;
use windows::core::{Interface, IUnknown, Vtable};
use windows::Win32::Devices::HumanInterfaceDevice::{
    DIDEVICEOBJECTDATA, DIMOUSESTATE, DIMOUSESTATE2, DIRECTINPUT_VERSION, GUID_SysMouse,
    IDirectInput8A, IDirectInputDevice8A,
};
use windows::Win32::Graphics::Direct3D9::{
    D3DBACKBUFFER_TYPE_MONO, D3DDEVICE_CREATION_PARAMETERS, D3DVIEWPORT9, IDirect3DDevice9,
    IDirect3DSurface9,
};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleFileNameA, GetModuleHandleA, GetProcAddress, LoadLibraryA,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcA, ClipCursor, CreateWindowExA, DefWindowProcA, DestroyWindow, GetCursorPos,
    GetWindowRect, LoadCursorW, SetCursor, SetWindowLongA, ShowCursor, WNDPROC, GWL_WNDPROC,
    IDC_ARROW, WM_CHAR, WM_INPUT, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN,
    WM_XBUTTONUP, XBUTTON1,
};

use crate::patch_utils::patch_bytes;
use crate::runtime_log::{log_error, log_info, log_warn};

const IDIRECT3D9_VTBL_CREATE_DEVICE_INDEX: usize = 16;
const IDIRECT3D9_VTBL_RELEASE_INDEX: usize = 2;
const IDIRECT3DDEVICE9_VTBL_RELEASE_INDEX: usize = 2;
const IDIRECT3DDEVICE9_VTBL_PRESENT_INDEX: usize = 17;
const IDIRECT3DDEVICE9_VTBL_BEGINSCENE_INDEX: usize = 41;
const IDIRECT3DDEVICE9_VTBL_RESET_INDEX: usize = 16;
const IDIRECT3DDEVICE9_VTBL_ENDSCENE_INDEX: usize = 42;
const IDIRECTINPUTDEVICE8_VTBL_GETDEVICESTATE_INDEX: usize = 9;
const IDIRECTINPUTDEVICE8_VTBL_GETDEVICEDATA_INDEX: usize = 10;

const D3D_SDK_VERSION: u32 = 32;
const D3DDEVTYPE_HAL: u32 = 1;
const D3DCREATE_SOFTWARE_VERTEXPROCESSING: u32 = 0x20;
const D3DSWAPEFFECT_DISCARD: u32 = 1;
const OVERLAY_TOGGLE_KEY_VK_F8: WPARAM = 0x77;
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_END: u32 = 0x23;
const VK_HOME: u32 = 0x24;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_INSERT: u32 = 0x2D;
const VK_DELETE: u32 = 0x2E;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_ALPHA_0: u32 = 0x30;
const VK_ALPHA_9: u32 = 0x39;
const VK_A: u32 = 0x41;
const VK_Z: u32 = 0x5A;
const VK_F1: u32 = 0x70;
const VK_F12: u32 = 0x7B;

#[repr(C)]
struct D3dPresentParameters {
    back_buffer_width: u32,
    back_buffer_height: u32,
    back_buffer_format: u32,
    back_buffer_count: u32,
    multi_sample_type: u32,
    multi_sample_quality: u32,
    swap_effect: u32,
    h_device_window: HWND,
    windowed: i32,
    enable_auto_depth_stencil: i32,
    auto_depth_stencil_format: u32,
    flags: u32,
    full_screen_refresh_rate_in_hz: u32,
    presentation_interval: u32,
}

type Direct3DCreate9Fn = unsafe extern "system" fn(sdk_version: u32) -> *mut c_void;
type D3D9CreateDeviceFn = unsafe extern "system" fn(
    this: *mut c_void,
    adapter: u32,
    device_type: u32,
    focus_window: HWND,
    behavior_flags: u32,
    presentation_parameters: *mut D3dPresentParameters,
    returned_device_interface: *mut *mut c_void,
) -> i32;
type D3D9ReleaseFn = unsafe extern "system" fn(this: *mut c_void) -> u32;
type BeginSceneFn = unsafe extern "system" fn(device: *mut c_void) -> i32;
type EndSceneFn = unsafe extern "system" fn(device: *mut c_void) -> i32;
type DirectInput8CreateFn = unsafe extern "system" fn(
    hinst: *mut c_void,
    version: u32,
    riidltf: *const windows::core::GUID,
    ppvout: *mut *mut c_void,
    punkouter: *mut c_void,
) -> i32;
type DiGetDeviceStateFn =
    unsafe extern "system" fn(this: *mut c_void, cb_data: u32, data: *mut c_void) -> i32;
type DiGetDeviceDataFn = unsafe extern "system" fn(
    this: *mut c_void,
    cb_object_data: u32,
    rgdod: *mut DIDEVICEOBJECTDATA,
    pdw_inout: *mut u32,
    flags: u32,
) -> i32;
type PresentFn = unsafe extern "system" fn(
    device: *mut c_void,
    src_rect: *const c_void,
    dst_rect: *const c_void,
    dst_window_override: HWND,
    dirty_region: *const c_void,
) -> i32;
type ResetFn =
    unsafe extern "system" fn(device: *mut c_void, params: *mut D3dPresentParameters) -> i32;

static D3D9_HOOKS_INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIG_PRESENT: AtomicUsize = AtomicUsize::new(0);
static ORIG_RESET: AtomicUsize = AtomicUsize::new(0);
static PRESENT_CALL_COUNT: AtomicU32 = AtomicU32::new(0);
static PANEL_ANTI_TAMPER_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_DLC_FIX_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_CAMERA_FIX_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_CAMERA_SHAKE_FIX_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_FOV_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_FOV_BITS: AtomicU32 = AtomicU32::new(0);
static OVERLAY_RETRY_AFTER_CALL: AtomicU32 = AtomicU32::new(0);
static OVERLAY_FIRST_SUCCESSFUL_RENDER_LOGGED: AtomicBool = AtomicBool::new(false);
static OVERLAY_BACKBUFFER_BIND_LOGGED: AtomicBool = AtomicBool::new(false);
static OVERLAY_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
static OVERLAY_INPUT_CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
static OVERLAY_TARGET_HWND_BITS: AtomicUsize = AtomicUsize::new(0);
static OVERLAY_WNDPROC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static OVERLAY_WNDPROC_HWND_BITS: AtomicUsize = AtomicUsize::new(0);
static OVERLAY_ORIGINAL_WNDPROC_BITS: AtomicUsize = AtomicUsize::new(0);
static OVERLAY_WNDPROC_INSTALL_FAILED_LOGGED: AtomicBool = AtomicBool::new(false);
static OVERLAY_CURSOR_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
static OVERLAY_SHOWCURSOR_DELTA: AtomicI32 = AtomicI32::new(0);
static DINPUT_HOOKS_INSTALLED: AtomicBool = AtomicBool::new(false);
static ORIG_DINPUT_GET_DEVICE_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_DINPUT_GET_DEVICE_DATA: AtomicUsize = AtomicUsize::new(0);

struct OverlayRenderState {
    imgui: imgui::Context,
    renderer: Renderer,
    last_frame: Instant,
}

thread_local! {
    static OVERLAY_RENDER_STATE: RefCell<Option<OverlayRenderState>> = const { RefCell::new(None) };
}

include!("overlay/shared.rs");
include!("overlay/wndproc.rs");
include!("overlay/dinput_hooks.rs");
include!("overlay/render_hooks.rs");
