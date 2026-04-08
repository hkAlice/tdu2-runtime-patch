use core::ffi::c_void;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

static VERSION_PROXY: OnceLock<Option<VersionProxy>> = OnceLock::new();

type GetFileVersionInfoAFn =
    unsafe extern "system" fn(*const u8, u32, u32, *mut c_void) -> BOOL;
type GetFileVersionInfoSizeAFn = unsafe extern "system" fn(*const u8, *mut u32) -> u32;
type GetFileVersionInfoSizeWFn = unsafe extern "system" fn(*const u16, *mut u32) -> u32;
type GetFileVersionInfoWFn =
    unsafe extern "system" fn(*const u16, u32, u32, *mut c_void) -> BOOL;
type VerFindFileAFn = unsafe extern "system" fn(
    u32,
    *const u8,
    *const u8,
    *const u8,
    *mut u8,
    *mut u32,
    *mut u8,
    *mut u32,
) -> u32;
type VerFindFileWFn = unsafe extern "system" fn(
    u32,
    *const u16,
    *const u16,
    *const u16,
    *mut u16,
    *mut u32,
    *mut u16,
    *mut u32,
) -> u32;
type VerInstallFileAFn = unsafe extern "system" fn(
    u32,
    *const u8,
    *const u8,
    *const u8,
    *const u8,
    *const u8,
    *mut u8,
    *mut u32,
) -> u32;
type VerInstallFileWFn = unsafe extern "system" fn(
    u32,
    *const u16,
    *const u16,
    *const u16,
    *const u16,
    *const u16,
    *mut u16,
    *mut u32,
) -> u32;
type VerLanguageNameAFn = unsafe extern "system" fn(u32, *mut u8, u32) -> u32;
type VerLanguageNameWFn = unsafe extern "system" fn(u32, *mut u16, u32) -> u32;
type VerQueryValueAFn =
    unsafe extern "system" fn(*const c_void, *const u8, *mut *mut c_void, *mut u32) -> BOOL;
type VerQueryValueWFn =
    unsafe extern "system" fn(*const c_void, *const u16, *mut *mut c_void, *mut u32) -> BOOL;

#[derive(Clone, Copy)]
struct VersionProxy {
    _module: usize,
    get_file_version_info_a: GetFileVersionInfoAFn,
    get_file_version_info_size_a: GetFileVersionInfoSizeAFn,
    get_file_version_info_size_w: GetFileVersionInfoSizeWFn,
    get_file_version_info_w: GetFileVersionInfoWFn,
    ver_find_file_a: VerFindFileAFn,
    ver_find_file_w: VerFindFileWFn,
    ver_install_file_a: VerInstallFileAFn,
    ver_install_file_w: VerInstallFileWFn,
    ver_language_name_a: VerLanguageNameAFn,
    ver_language_name_w: VerLanguageNameWFn,
    ver_query_value_a: VerQueryValueAFn,
    ver_query_value_w: VerQueryValueWFn,
}

unsafe fn resolve_proc_address(module: *mut c_void, symbol: &'static [u8]) -> Option<usize> {
    GetProcAddress(module, symbol.as_ptr()).map(|proc| proc as usize)
}

macro_rules! resolve_typed_proc {
    ($module:expr, $symbol:expr, $ty:ty) => {{
        match resolve_proc_address($module, $symbol) {
            Some(proc) => Some(std::mem::transmute::<usize, $ty>(proc)),
            None => None,
        }
    }};
}

unsafe extern "system" fn ver_install_file_a_stub(
    _: u32,
    _: *const u8,
    _: *const u8,
    _: *const u8,
    _: *const u8,
    _: *const u8,
    _: *mut u8,
    _: *mut u32,
) -> u32 {
    0
}

unsafe extern "system" fn ver_install_file_w_stub(
    _: u32,
    _: *const u16,
    _: *const u16,
    _: *const u16,
    _: *const u16,
    _: *const u16,
    _: *mut u16,
    _: *mut u32,
) -> u32 {
    0
}

unsafe fn initialize_version_proxy() -> Option<VersionProxy> {
    let kernelbase = GetModuleHandleA(b"kernelbase.dll\0".as_ptr());
    let kernel32 = GetModuleHandleA(b"kernel32.dll\0".as_ptr());

    if kernelbase.is_null() && kernel32.is_null() {
        return None;
    }

    let get_file_version_info_a = match resolve_typed_proc!(
        kernelbase,
        b"GetFileVersionInfoA\0",
        GetFileVersionInfoAFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"GetFileVersionInfoA\0", GetFileVersionInfoAFn))
    {
        Some(func) => func,
        None => return None,
    };

    let get_file_version_info_size_a = match resolve_typed_proc!(
        kernelbase,
        b"GetFileVersionInfoSizeA\0",
        GetFileVersionInfoSizeAFn
    )
    .or_else(|| {
        resolve_typed_proc!(
            kernel32,
            b"GetFileVersionInfoSizeA\0",
            GetFileVersionInfoSizeAFn
        )
    }) {
        Some(func) => func,
        None => return None,
    };

    let get_file_version_info_size_w = match resolve_typed_proc!(
        kernelbase,
        b"GetFileVersionInfoSizeW\0",
        GetFileVersionInfoSizeWFn
    )
    .or_else(|| {
        resolve_typed_proc!(
            kernel32,
            b"GetFileVersionInfoSizeW\0",
            GetFileVersionInfoSizeWFn
        )
    }) {
        Some(func) => func,
        None => return None,
    };

    let get_file_version_info_w = match resolve_typed_proc!(
        kernelbase,
        b"GetFileVersionInfoW\0",
        GetFileVersionInfoWFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"GetFileVersionInfoW\0", GetFileVersionInfoWFn))
    {
        Some(func) => func,
        None => return None,
    };

    let ver_find_file_a = match resolve_typed_proc!(kernelbase, b"VerFindFileA\0", VerFindFileAFn)
        .or_else(|| resolve_typed_proc!(kernel32, b"VerFindFileA\0", VerFindFileAFn))
    {
        Some(func) => func,
        None => return None,
    };

    let ver_find_file_w = match resolve_typed_proc!(kernelbase, b"VerFindFileW\0", VerFindFileWFn)
        .or_else(|| resolve_typed_proc!(kernel32, b"VerFindFileW\0", VerFindFileWFn))
    {
        Some(func) => func,
        None => return None,
    };

    let ver_install_file_a =
        resolve_typed_proc!(kernelbase, b"VerInstallFileA\0", VerInstallFileAFn)
            .or_else(|| resolve_typed_proc!(kernel32, b"VerInstallFileA\0", VerInstallFileAFn))
            .unwrap_or(ver_install_file_a_stub);

    let ver_install_file_w =
        resolve_typed_proc!(kernelbase, b"VerInstallFileW\0", VerInstallFileWFn)
            .or_else(|| resolve_typed_proc!(kernel32, b"VerInstallFileW\0", VerInstallFileWFn))
            .unwrap_or(ver_install_file_w_stub);

    let ver_language_name_a = match resolve_typed_proc!(
        kernelbase,
        b"VerLanguageNameA\0",
        VerLanguageNameAFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"VerLanguageNameA\0", VerLanguageNameAFn)) {
        Some(func) => func,
        None => return None,
    };

    let ver_language_name_w = match resolve_typed_proc!(
        kernelbase,
        b"VerLanguageNameW\0",
        VerLanguageNameWFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"VerLanguageNameW\0", VerLanguageNameWFn)) {
        Some(func) => func,
        None => return None,
    };

    let ver_query_value_a = match resolve_typed_proc!(
        kernelbase,
        b"VerQueryValueA\0",
        VerQueryValueAFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"VerQueryValueA\0", VerQueryValueAFn)) {
        Some(func) => func,
        None => return None,
    };

    let ver_query_value_w = match resolve_typed_proc!(
        kernelbase,
        b"VerQueryValueW\0",
        VerQueryValueWFn
    )
    .or_else(|| resolve_typed_proc!(kernel32, b"VerQueryValueW\0", VerQueryValueWFn)) {
        Some(func) => func,
        None => return None,
    };

    Some(VersionProxy {
        _module: if !kernelbase.is_null() {
            kernelbase as usize
        } else {
            kernel32 as usize
        },
        get_file_version_info_a,
        get_file_version_info_size_a,
        get_file_version_info_size_w,
        get_file_version_info_w,
        ver_find_file_a,
        ver_find_file_w,
        ver_install_file_a,
        ver_install_file_w,
        ver_language_name_a,
        ver_language_name_w,
        ver_query_value_a,
        ver_query_value_w,
    })
}

fn version_proxy() -> Option<&'static VersionProxy> {
    VERSION_PROXY
        .get_or_init(|| unsafe { initialize_version_proxy() })
        .as_ref()
}

#[no_mangle]
pub unsafe extern "system" fn GetFileVersionInfoA(
    lptstr_filename: *const u8,
    dw_handle: u32,
    dw_len: u32,
    lp_data: *mut c_void,
) -> BOOL {
    match version_proxy() {
        Some(proxy) => {
            (proxy.get_file_version_info_a)(lptstr_filename, dw_handle, dw_len, lp_data)
        }
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn GetFileVersionInfoSizeA(
    lptstr_filename: *const u8,
    lpdw_handle: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.get_file_version_info_size_a)(lptstr_filename, lpdw_handle),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn GetFileVersionInfoSizeW(
    lptstr_filename: *const u16,
    lpdw_handle: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.get_file_version_info_size_w)(lptstr_filename, lpdw_handle),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn GetFileVersionInfoW(
    lptstr_filename: *const u16,
    dw_handle: u32,
    dw_len: u32,
    lp_data: *mut c_void,
) -> BOOL {
    match version_proxy() {
        Some(proxy) => {
            (proxy.get_file_version_info_w)(lptstr_filename, dw_handle, dw_len, lp_data)
        }
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerFindFileA(
    u_flags: u32,
    sz_file_name: *const u8,
    sz_win_dir: *const u8,
    sz_app_dir: *const u8,
    sz_cur_dir: *mut u8,
    lpu_cur_dir_len: *mut u32,
    sz_dest_dir: *mut u8,
    lpu_dest_dir_len: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_find_file_a)(
            u_flags,
            sz_file_name,
            sz_win_dir,
            sz_app_dir,
            sz_cur_dir,
            lpu_cur_dir_len,
            sz_dest_dir,
            lpu_dest_dir_len,
        ),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerFindFileW(
    u_flags: u32,
    sz_file_name: *const u16,
    sz_win_dir: *const u16,
    sz_app_dir: *const u16,
    sz_cur_dir: *mut u16,
    lpu_cur_dir_len: *mut u32,
    sz_dest_dir: *mut u16,
    lpu_dest_dir_len: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_find_file_w)(
            u_flags,
            sz_file_name,
            sz_win_dir,
            sz_app_dir,
            sz_cur_dir,
            lpu_cur_dir_len,
            sz_dest_dir,
            lpu_dest_dir_len,
        ),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerInstallFileA(
    u_flags: u32,
    sz_src_file_name: *const u8,
    sz_dest_file_name: *const u8,
    sz_src_dir: *const u8,
    sz_dest_dir: *const u8,
    sz_cur_dir: *const u8,
    sz_tmp_file: *mut u8,
    lpu_tmp_file_len: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_install_file_a)(
            u_flags,
            sz_src_file_name,
            sz_dest_file_name,
            sz_src_dir,
            sz_dest_dir,
            sz_cur_dir,
            sz_tmp_file,
            lpu_tmp_file_len,
        ),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerInstallFileW(
    u_flags: u32,
    sz_src_file_name: *const u16,
    sz_dest_file_name: *const u16,
    sz_src_dir: *const u16,
    sz_dest_dir: *const u16,
    sz_cur_dir: *const u16,
    sz_tmp_file: *mut u16,
    lpu_tmp_file_len: *mut u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_install_file_w)(
            u_flags,
            sz_src_file_name,
            sz_dest_file_name,
            sz_src_dir,
            sz_dest_dir,
            sz_cur_dir,
            sz_tmp_file,
            lpu_tmp_file_len,
        ),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerLanguageNameA(
    w_lang: u32,
    sz_lang: *mut u8,
    cch_lang: u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_language_name_a)(w_lang, sz_lang, cch_lang),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerLanguageNameW(
    w_lang: u32,
    sz_lang: *mut u16,
    cch_lang: u32,
) -> u32 {
    match version_proxy() {
        Some(proxy) => (proxy.ver_language_name_w)(w_lang, sz_lang, cch_lang),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerQueryValueA(
    p_block: *const c_void,
    lp_sub_block: *const u8,
    lplp_buffer: *mut *mut c_void,
    pu_len: *mut u32,
) -> BOOL {
    match version_proxy() {
        Some(proxy) => (proxy.ver_query_value_a)(p_block, lp_sub_block, lplp_buffer, pu_len),
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "system" fn VerQueryValueW(
    p_block: *const c_void,
    lp_sub_block: *const u16,
    lplp_buffer: *mut *mut c_void,
    pu_len: *mut u32,
) -> BOOL {
    match version_proxy() {
        Some(proxy) => (proxy.ver_query_value_w)(p_block, lp_sub_block, lplp_buffer, pu_len),
        None => 0,
    }
}
