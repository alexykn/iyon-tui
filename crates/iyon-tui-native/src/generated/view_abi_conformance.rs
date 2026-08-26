// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = ba12318e83f692e82b105219fcfcc8a53da1db568f007d381e091b259c67bc5c
// generator_blake3 = 0b79288012c798d4cc0a43433ed9113b1e50283de95f86b6059047a63a057665
pub(super) unsafe fn invoke_iyon_abi_conformance_u8_8_v1(
    a0: u8,
    a1: u8,
    a2: u8,
    a3: u8,
    a4: u8,
    a5: u8,
    a6: u8,
    a7: u8,
) -> u32 {
    u32::from(a0)
        .wrapping_mul(3)
        .wrapping_add(u32::from(a1).wrapping_mul(5))
        .wrapping_add(u32::from(a2).wrapping_mul(7))
        .wrapping_add(u32::from(a3).wrapping_mul(11))
        .wrapping_add(u32::from(a4).wrapping_mul(13))
        .wrapping_add(u32::from(a5).wrapping_mul(17))
        .wrapping_add(u32::from(a6).wrapping_mul(19))
        .wrapping_add(u32::from(a7).wrapping_mul(23))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_u8_8_v1(
    a0: u8,
    a1: u8,
    a2: u8,
    a3: u8,
    a4: u8,
    a5: u8,
    a6: u8,
    a7: u8,
) -> u32 {
    unsafe { invoke_iyon_abi_conformance_u8_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_u16_8_v1(
    a0: u16,
    a1: u16,
    a2: u16,
    a3: u16,
    a4: u16,
    a5: u16,
    a6: u16,
    a7: u16,
) -> u32 {
    u32::from(a0)
        .wrapping_mul(3)
        .wrapping_add(u32::from(a1).wrapping_mul(5))
        .wrapping_add(u32::from(a2).wrapping_mul(7))
        .wrapping_add(u32::from(a3).wrapping_mul(11))
        .wrapping_add(u32::from(a4).wrapping_mul(13))
        .wrapping_add(u32::from(a5).wrapping_mul(17))
        .wrapping_add(u32::from(a6).wrapping_mul(19))
        .wrapping_add(u32::from(a7).wrapping_mul(23))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_u16_8_v1(
    a0: u16,
    a1: u16,
    a2: u16,
    a3: u16,
    a4: u16,
    a5: u16,
    a6: u16,
    a7: u16,
) -> u32 {
    unsafe { invoke_iyon_abi_conformance_u16_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_u32_8_v1(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
) -> u32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
        .wrapping_add(a4.wrapping_mul(13))
        .wrapping_add(a5.wrapping_mul(17))
        .wrapping_add(a6.wrapping_mul(19))
        .wrapping_add(a7.wrapping_mul(23))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_u32_8_v1(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
) -> u32 {
    unsafe { invoke_iyon_abi_conformance_u32_8_v1(a0, a1, a2, a3, a4, a5, a6, a7) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_u32_16_v1(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
    a8: u32,
    a9: u32,
    a10: u32,
    a11: u32,
    a12: u32,
    a13: u32,
    a14: u32,
    a15: u32,
) -> u32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
        .wrapping_add(a4.wrapping_mul(13))
        .wrapping_add(a5.wrapping_mul(17))
        .wrapping_add(a6.wrapping_mul(19))
        .wrapping_add(a7.wrapping_mul(23))
        .wrapping_add(a8.wrapping_mul(29))
        .wrapping_add(a9.wrapping_mul(31))
        .wrapping_add(a10.wrapping_mul(37))
        .wrapping_add(a11.wrapping_mul(41))
        .wrapping_add(a12.wrapping_mul(43))
        .wrapping_add(a13.wrapping_mul(47))
        .wrapping_add(a14.wrapping_mul(53))
        .wrapping_add(a15.wrapping_mul(59))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_u32_16_v1(
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
    a8: u32,
    a9: u32,
    a10: u32,
    a11: u32,
    a12: u32,
    a13: u32,
    a14: u32,
    a15: u32,
) -> u32 {
    unsafe {
        invoke_iyon_abi_conformance_u32_16_v1(
            a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15,
        )
    }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_i32_4_v1(
    a0: i32,
    a1: i32,
    a2: i32,
    a3: i32,
) -> i32 {
    a0.wrapping_mul(3)
        .wrapping_add(a1.wrapping_mul(5))
        .wrapping_add(a2.wrapping_mul(7))
        .wrapping_add(a3.wrapping_mul(11))
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_i32_4_v1(a0: i32, a1: i32, a2: i32, a3: i32) -> i32 {
    unsafe { invoke_iyon_abi_conformance_i32_4_v1(a0, a1, a2, a3) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_f32_4_v1(
    a0: f32,
    a1: f32,
    a2: f32,
    a3: f32,
) -> f32 {
    a0 * 3.0 + a1 * 5.0 + a2 * 7.0 + a3 * 11.0
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_f32_4_v1(a0: f32, a1: f32, a2: f32, a3: f32) -> f32 {
    unsafe { invoke_iyon_abi_conformance_f32_4_v1(a0, a1, a2, a3) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_f64_4_v1(
    a0: f64,
    a1: f64,
    a2: f64,
    a3: f64,
) -> f64 {
    a0 * 3.0 + a1 * 5.0 + a2 * 7.0 + a3 * 11.0
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_f64_4_v1(a0: f64, a1: f64, a2: f64, a3: f64) -> f64 {
    unsafe { invoke_iyon_abi_conformance_f64_4_v1(a0, a1, a2, a3) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_pointer_v1(a0: *mut ::core::ffi::c_void) -> u32 {
    if a0.is_null() { 0 } else { 1 }
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_pointer_v1(a0: *mut ::core::ffi::c_void) -> u32 {
    unsafe { invoke_iyon_abi_conformance_pointer_v1(a0) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_buffer_v1(a0: *const u8, a1: usize) -> u32 {
    if a0.is_null() {
        u32::MAX
    } else {
        (a1 as u32)
            .wrapping_mul(257)
            .wrapping_add(unsafe { *a0 as u32 })
    }
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_buffer_v1(a0: *const u8, a1: usize) -> u32 {
    unsafe { invoke_iyon_abi_conformance_buffer_v1(a0, a1) }
}

pub(super) unsafe fn invoke_iyon_abi_conformance_cstring_v1(a0: *const ::core::ffi::c_char) -> u32 {
    if a0.is_null() {
        0
    } else {
        unsafe { ::core::ffi::CStr::from_ptr(a0) }
            .to_bytes()
            .iter()
            .fold(2166136261u32, |hash, byte| {
                hash.wrapping_mul(16777619).wrapping_add(u32::from(*byte))
            })
    }
}

#[cfg(feature = "direct-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iyon_abi_conformance_cstring_v1(a0: *const ::core::ffi::c_char) -> u32 {
    unsafe { invoke_iyon_abi_conformance_cstring_v1(a0) }
}
