use napi_derive::napi;

/// Stable framework marker proving Bun loaded this TUI-native boundary.
#[napi(js_name = "nativeVersion")]
pub fn native_version() -> String {
    "iyon-tui-native/s6".to_owned()
}
