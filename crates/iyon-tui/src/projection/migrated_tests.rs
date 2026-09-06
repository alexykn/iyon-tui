// Projection API behavior is tested inside the private owner. These tests
// retain the old semantic assertions without preserving an external Rust
// authoring facade for the unpublished runtime crate.

mod p3c_ergonomics {
    use crate as iyon_tui;

    include!("tests/p3c_ergonomics.rs");
}

mod projection_public {
    use crate as iyon_tui;

    include!("tests/projection_public.rs");
}
