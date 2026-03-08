# AGENTS.md — ebkit_rs

**Generated:** 2026-02-08 | **Branch:** main (pre-initial-commit)

## OVERVIEW

Rust workspacebfor reading, parsing, and converting Prophesee event-camera recordings.
Edition 2024, stable toolchain. Workspace members match `ebkit_*` glob — no source
code exists yet; the project is greenfield with detailed format specifications in `spec/`.

## GOALS & NON-GOALS

### Goals
1. **Event camera data reader/parser** — decode `.raw` files containing EVT 2.0/2.1/3.0/4.0 streams
2. **WASM-friendly** — core parsing must compile to `wasm32-unknown-unknown`
3. **Streaming parser with seek** — play event data like video (seek by timestamp, stream forward)
4. **RAW → HDF5 conversion** — convert `.raw` event recordings into HDF5 with ECF compression

### Non-Goals
- ~~HDF5 → RAW~~ — no reverse conversion
- ~~Realtime camera~~ — no live sensor access (OpenEB handles that)
- ~~Python bindings~~ — use OpenEB's Python SDK instead

## STRUCTURE

```
ebkit_rs/
├── Cargo.toml         # Workspace root: members = ["ebkit_*"], resolver = "3"
├── Cargo.lock
├── spec/              # Format specifications (derived from Prophesee docs + OpenEB)
│   ├── README.md      # Spec index with format hierarchy diagram
│   ├── raw.md         # .raw container: ASCII header + binary event stream
│   ├── evt20.md        # EVT 2.0: 32-bit non-vectorized, 34-bit timestamps
│   ├── evt21.md       # EVT 2.1: 64-bit vectorized (32-pixel groups)
│   ├── evt30.md        # EVT 3.0: 16-bit stateful vectorized, 24-bit timestamps
│   ├── evt40.md        # EVT 4.0: 32-bit vectorized, scalar + vector CD, 34-bit timestamps
│   └── hdf5.md        # HDF5 container: ECF-compressed decoded events + indexes
├── openeb/            # Vendored OpenEB C++ SDK — DO NOT MODIFY (1436 files)
└── target/            # Build output (gitignored)
```

**No `ebkit_*` member crates exist yet.** Workspace is scaffolded but source code is TBD.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Understand file formats | `spec/README.md` | Start here — has hierarchy diagram and cross-reference table |
| RAW header parsing | `spec/raw.md` | ASCII `% key value\n` header, `% end` terminator, `format` field syntax |
| EVT 2.0 decoding | `spec/evt20.md` | 32-bit words, 4-bit type tag at `[31:28]`, 34-bit timestamps |
| EVT 2.1 decoding | `spec/evt21.md` | 64-bit words, IMX636 half-swap caveat, 32-pixel vectorization |
| EVT 3.0 decoding | `spec/evt30.md` | 16-bit stateful decoder, 24-bit timestamps, VECT_12/VECT_8 |
| EVT 4.0 decoding | `spec/evt40.md` | 32-bit words, scalar + vector CD, 34-bit timestamps, ERC counters |
| HDF5 output format | `spec/hdf5.md` | ECF filter 0x8ECF, compound type `{x:u16, y:u16, p:i16, t:i64}` |
| C++ reference decoders | `openeb/hal/cpp/include/metavision/hal/decoders/` | EVT 2.x/3.0 C++ struct layouts |
| C++ standalone EVT2 decoder | `openeb/standalone_samples/metavision_evt2_raw_file_decoder/` | Self-contained EVT2 decoder with inline format docs |
| C++ standalone EVT3 decoder | `openeb/standalone_samples/metavision_evt3_raw_file_decoder/` | Self-contained EVT3 decoder with inline format docs |
| C++ decoder factory | `openeb/hal/cpp/src/utils/make_decoder.cpp` | Maps format strings → decoder classes |
| C++ raw file header | `openeb/hal/cpp/include/metavision/hal/utils/raw_file_header.h` | Reference parser |
| C++ generic header | `openeb/sdk/modules/base/cpp/src/generic_header.cpp` | Header parsing algorithm |
| C++ EVT4 decoder | `openeb/hal/cpp/include/metavision/hal/decoders/evt4/` | EVT4 event types, decoder, and validator |
| C++ EVT2/3 test formats | `openeb/hal/cpp/test/gtest_utils/evt{2,3}_raw_format.h` | Bitfield struct definitions for test harness |

## FORMAT QUICK REFERENCE

### Container Formats
- **`.raw`** — ASCII header (`% key value\n`) + raw binary event stream (EVT 2.0/2.1/3.0/4.0)
- **`.hdf5`** — HDF5 groups (`CD/`, `EXT_TRIGGER/`) with ECF-compressed decoded events + timestamp indexes

### Event Stream Encodings

| Format | Word Size | Vectorized | Stateful | Timestamp Bits | Max Events/Word |
|--------|-----------|------------|----------|----------------|-----------------|
| EVT 2.0 | 32-bit | No | No | 34 (28+6) | 1 |
| EVT 2.1 | 64-bit | Yes (32px) | No | 34 (28+6) | 32 |
| EVT 3.0 | 16-bit | Yes (12/8) | **Yes** | 24 (12+12) | 12 |
| EVT 4.0 | 32-bit | Yes (32px) | No | 34 (28+6) | 1 (scalar) / 32 (vector) |

### Critical Implementation Details
- **Byte order:** Little-endian for all current sensors (IMX636, GenX320)
- **EVT 2.1 IMX636 caveat:** 64-bit words sent as two LE 32-bit halves, upper first — must swap before interpreting
- **EVT 3.0 state:** Decoder maintains `{time_high, time_low, y, base_x, polarity, system_type}`
- **Timestamps:** Microseconds throughout. EVT 2.x rolls over at ~4h46m; EVT 3.0 at ~16.78s
- **EVT 4.0 type codes:** Completely different from EVT 2.x (CD_OFF=0xA, CD_ON=0xB, TIME_HIGH=0xE, PADDING=0xF)
- **EVT 4.0 vectorized CD:** Two 32-bit words (header + 32-bit bitmask), up to 32 events per pair
- **RAW header `format` field:** `EVT3;height=720;width=1280` or `EVT4;height=...` — primary field for selecting decoder
- **HDF5 ECF filter code:** `0x8ECF` (36559 decimal), requires plugin at `HDF5_PLUGIN_PATH`
- **HDF5 index interval:** Every 2000 µs; `offset` attribute adjusts stored timestamps

## WORKSPACE DEPENDENCIES

```toml
[workspace.dependencies]
anyhow = "1"      # Application error handling (binary crates)
thiserror = "2"   # Library error types (derive Error)
```

## CONVENTIONS

### Edition 2024 Specifics
- `unsafe_op_in_unsafe_fn` is deny-by-default — wrap all unsafe ops in `unsafe { }` even inside `unsafe fn`
- `gen` is a reserved keyword — do not use as identifier
- Lifetime elision: `impl Trait + '_` semantics apply

### Code Style
- `cargo fmt` defaults, no custom rustfmt.toml
- Imports: `std` → external crates → `crate::`/`super::`, separated by blank lines
- Granular imports (no globs except preludes)
- Strong typing: newtypes for domain concepts (timestamps, coordinates, polarity)
- `thiserror` for library errors, `anyhow` for binary crate errors
- `?` propagation, `.context("...")` at boundaries
- No lossy `as` casts — use `TryFrom`/`TryInto`

### WASM Constraints
- Core parsing crate(s) MUST NOT depend on `std::fs`, `std::net`, or OS-specific APIs
- Use `#[cfg(target_arch = "wasm32")]` for platform-specific paths
- I/O abstraction: accept `impl Read + Seek` (or equivalent trait) — not file paths
- No threads in WASM — streaming design must work single-threaded

### Performance
- Hot path: high-throughput event stream decoding (millions of events/sec)
- Zero-copy where possible — borrow over clone in data pipelines
- Iterators over allocation in tight loops
- Profile before optimizing (`cargo bench`, `perf`, `flamegraph`)

### Documentation (docs-as-code)
- Public items MUST have `///` doc comments
- Module-level `//!` at top of file
- `# Examples`, `# Errors`, `# Panics` sections where applicable
- **Only necessary comments** — code should be self-documenting through clear naming, types, and structure
- Comment the **why**, never the **what** — if code needs a "what" comment, refactor it instead
- No commented-out code — use version control
- No boilerplate/placeholder comments (`// TODO: implement`, `// default`, etc.) unless tied to a tracked issue

### Safety
- Every `unsafe` block MUST have `// SAFETY: ...` comment
- Wrap unsafe in safe public APIs with documented preconditions

### Testing
- Unit tests in `#[cfg(test)] mod tests { }` at bottom of file
- Integration tests in `tests/`
- Test names: `test_<behavior>` or `<behavior>_should_<expected>`
- `assert_eq!`/`assert_ne!` over bare `assert!`

## ANTI-PATTERNS

- **DO NOT** modify anything under `openeb/` — vendored reference only
- **DO NOT** use `as` for numeric casts — use `TryFrom`/`TryInto`
- **DO NOT** swallow errors: `let _ = fallible()` forbidden without comment
- **DO NOT** use `unwrap()`/`expect()` except for provably impossible failures (document why)
- **DO NOT** introduce `std::fs` dependencies in core parsing crates (breaks WASM)
- **DO NOT** convert HDF5 back to RAW format (explicit non-goal)
- **DO NOT** add Python bindings (use OpenEB's Python SDK)
- **DO NOT** implement realtime camera access (use OpenEB)

## COMMANDS

```bash
# Build
cargo build                    # dev
cargo build --release          # optimized

# Lint (pre-commit)
cargo fmt -- --check && cargo clippy --all-targets -- -D warnings

# Test
cargo test                     # all tests
cargo test -- --nocapture      # show stdout
RUST_BACKTRACE=1 cargo test    # with backtrace

# Format
cargo fmt                      # ALWAYS before committing
```

## NOTES

- Project is **greenfield** — no source code exists yet. Workspace root is scaffolded with `members = ["ebkit_*"]` expecting member crates like `ebkit_core`, `ebkit_cli`, etc.
- The `spec/` directory is the primary design reference. All format specs include pseudocode decoding algorithms and C++ struct references from OpenEB.
- EVT 3.0 is the most complex format (stateful decoder with 6 state variables) and likely the highest priority since IMX636/GenX320 cameras use it.
- Seeking in RAW files requires building a timestamp→byte-offset index (similar to `.raw.tmp_index` sidecar in OpenEB). HDF5 has built-in indexes every 2000 µs.
- ECF compression codec is open-source at [github.com/prophesee-ai/hdf5_ecf](https://github.com/prophesee-ai/hdf5_ecf) — will need Rust implementation or FFI for HDF5 writing.
