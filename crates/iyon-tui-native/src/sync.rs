use napi_derive::napi;

/// Stable framework marker proving Bun loaded this TUI-native bridge.
#[napi(js_name = "nativeVersion")]
pub fn native_version() -> String {
    "iyon-tui-native/s3".to_owned()
}
