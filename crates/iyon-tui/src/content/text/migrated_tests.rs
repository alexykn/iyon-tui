// These semantic regression tests used to be integration tests that treated
// the unpublished Rust crate as an authoring package. Keep their behavioral
// coverage inside the owning module instead; the public authoring contract is
// the TypeScript facade and the native crate crosses only `binding`.

mod document_public {
    use crate as iyon_tui;

    include!("tests/document_public.rs");
}

mod markdown_composition {
    use crate as iyon_tui;

    include!("tests/markdown_composition.rs");
}

mod markdown_hardening {
    use crate as iyon_tui;

    include!("tests/markdown_hardening.rs");
}

mod markdown_incremental {
    use crate as iyon_tui;

    include!("tests/markdown_incremental.rs");
}

mod markdown_smoke {
    use crate as iyon_tui;

    include!("tests/markdown_smoke.rs");
}

mod pulldown_characterization {
    include!("tests/pulldown_characterization.rs");
}

mod text_origin {
    use crate as iyon_tui;

    include!("tests/text_origin.rs");
}
