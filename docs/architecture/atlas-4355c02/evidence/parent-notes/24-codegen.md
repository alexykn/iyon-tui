# Parent integration notes — 24 codegen

Fully read all 1517 lines in ranges 1–500, 501–1000, 1001–1517. Static scout; no execution evidence.

## Integrated model
- TOML schema + separate view-kind-codes.json feed private Rust generator, 16 checked outputs: six Rust generated files (types,exports,conformance,table,NAPI,state schema), C view header, five TS generated transport/state files, generated Rust+TS tests, benchmark registry, historical-path human reference. Content C ABI header is handwritten and outside generator; do not conflate generated structural NAPI route with direct-FFI content ingress.
- 60 structural functions, 10 conformance probes. Schema ABI1/semantic1, Bun minimum+qualified1.4.0, high-bit u32 status. Session validates name/version/fingerprints/transport NAPI/generation/count then noop. Signed raw statuses require caller checks, generated checkedRef only ref results.
- Schema owns function/lowering/range/bounds/thread/borrow descriptions and scaffolding; handwritten native owns lifetime, leases, semantic caches, constructors, host acceptance, state legality/wakes. Pointer nonnull validation is NOT live-registry validation (17 correction).
- State17 properties: geometry10 mask3ff nullable278 words14 strings0; presentation7 mask7f nullable5f words5 strings14. Dense IDs, lane shape, generated masks/packers/check envelopes; handwritten normalizers/readers own meaning, errors, capabilities.
- Fixed arity/depth and buffers/builders/edit transactions coexist; feature-gated structural C ABI is qualification, production structural TS uses safe NAPI session. fast-view-abi suppresses panic wrapper, build qualifications matter.
- check mode byte-compares all16; generated tests mostly schema/generated agreement and stub delegation, not independent production semantics. Ownership gate existence/import boundaries not exhaustive semantic proof.

## Independent source checks and reconciliation
Read complete tools/tui-abi-gen/src/render_manifest.rs and main.rs render_outputs/check path (~181–290). Confirmed manifest args OMIT buffer_used_of even though buffer_length_of serialized; manifest enum entries carry source_key, NOT resolved numeric values. This is metadata incompleteness, not established execution error.
Additional parent finding: schema_hash hashes TOML source ONLY. generator_hash concatenates included generator Rust/template bytes ONLY. Separate view-kind-codes.json is loaded and used for output generation but neither hash covers its content. Therefore do not call runtime fingerprint checks a complete proof of enum/kind-code input equality. Generator byte freshness can catch affected generated-output drift; runtime same-fingerprint check by itself cannot identify a kind-code-only change. No mismatched deployed artifact or runtime defect demonstrated. Need group with 29/41/43.
Report's 'every generated output carries' hashes broadly plausible but avoid extending hash guarantees to all inputs. Metadata descriptions do not mechanically enforce every ownership/thread semantic; handwritten runtime validation remains authoritative.

## Synthesis references
Use report inventory for all16 paths, schema contracts, output consumers, CLI generate/check/printmanifest/explain, validation vocabulary, conformance probes, generated vs handwritten boundaries. Preserve no tests executed status; historical CI claims are not current run results.
