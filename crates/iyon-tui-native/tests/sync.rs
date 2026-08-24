use iyon_tui_native::{native_version, tui_smoke};

#[test]
fn framework_native_markers_are_stable() {
    assert_eq!(native_version(), "iyon-tui-native/s3");
    assert_eq!(
        tui_smoke().expect("TUI framework probe should succeed"),
        "iyon-tui/t1"
    );
}
