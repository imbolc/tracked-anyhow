# Automatic error location tracking

## Purpose and scope

Make `tracked-anyhow` a drop-in replacement for `anyhow` that produces
location-aware diagnostics resembling [anyhow-auto-context], without changing
application imports, public types, method signatures, macro syntax, or ordinary
`?` expressions.

- Keep the public API unchanged and upstream `anyhow` commits easy to merge
- Follow upstream anyhow's diagnostic format as closely as possible, adding only
  metadata annotations needed for diagnosis, with compact `[file:line:column]`
  suffixes for locations; format compatibility takes precedence over resemblance
  to `anyhow-auto-context`
- Capture creation, conversion, and explicit context-attachment locations
- Enrich errors created by existing macros with source text and best-effort
  enclosing scope
- The code diff with anyhow should be minimal

This is not a complete error-propagation trace: forwarding an existing anyhow
error with `?` does not enter this crate.

The original dependency-rename sketch illustrates the goal of changing only
Cargo configuration:

```diff
- anyhow = "1.0"
+ anyhow = { package = "tracked-anyhow", version = "0.1" }
```

Essentially it should keep anyhow behaviour intact, only adding compact
bracketed location to the error formatting:

```text
loading configuration [src/main.rs:28]

Caused by:
    No such file or directory (os error 2) [src/config.rs:12]
```

This sketch does not satisfy the package-identity requirement. For adoption,
keep the package name `anyhow` and use the workspace-root patch described in
[Adoption and excluded scope](#adoption-and-excluded-scope).

## Compatibility contract

- Keep `anyhow::Result` an alias for `core::result::Result` and preserve
  existing public signatures, trait bounds, macro forms, and feature-dependent
  API availability; do not require new traits, macros, attributes, or accessors
- Preserve `{}`, `{:#}`, `to_string()`, and the existing `{:#?}` delegation to
  the underlying error; enrich normal `{:?}` diagnostics only, including the
  report produced by an error returned from `main`
- Keep normal `{:?}` recognizable as upstream anyhow output; add inline
  bracketed annotations without adding lines, reorganizing the report, or
  changing cause numbering
- Preserve source-chain contents and order, `root_cause()`, dereferencing, typed
  context, `is`, and downcasting by value, shared reference, and mutable
  reference
- Preserve the one-word `Error` representation, its existing `Option<Error>`
  layout, and auto traits; metadata may enlarge the private heap allocation
- Preserve the minimum supported Rust version (MSRV), `no_std` support, and
  existing backtrace capture policy
- Preserve success-path evaluation behavior; keep `with_context` lazy and
  evaluate its closure exactly once on failure

Location tracking has these configuration and capture requirements:

- Enable tracking without additional user configuration; do not add a feature
  flag or runtime setting in this change
- Support locations with backtraces disabled; existing backtraces remain
  independent diagnostics
- Do not perform stack walks, symbolization, or source-file reads when capturing
  location metadata

Hidden helpers needed by exported macros may use the existing `__private`
namespace. They are implementation details, not a new documented API.

## Capture coverage

An **entry location** identifies where a message becomes an anyhow error or a
foreign error enters anyhow. A **context location** identifies where explicit
context is attached. Neither identifies an earlier failure inside a dependency.

| Existing operation                                                                      | Required location          | Additional information                         |
| --------------------------------------------------------------------------------------- | -------------------------- | ---------------------------------------------- |
| `anyhow!`, `format_err!`, or `bail!` creates an error                                   | Macro invocation           | Source tokens and best-effort scope            |
| A failing `ensure!` creates an error                                                    | Macro invocation           | Condition/message tokens and best-effort scope |
| Direct `Error::msg`                                                                     | Creation call              | Location only                                  |
| Direct `Error::new`, `Error::from`, or `Error::from_boxed`                              | Conversion call            | Location only                                  |
| Foreign error conversion through blanket `Into` or `?`                                  | Conversion call            | Subject to caller-location forwarding          |
| `Result::context`, `Result::with_context`, `Option::context`, or `Option::with_context` | Attachment call on failure | Location only                                  |
| `Error::context`                                                                        | Attachment call            | Preserve inner records                         |
| `?`, identity conversion, or direct return of an existing anyhow error                  | No new record              | Preserve existing records                      |
| `anyhow!(existing_error)` or `bail!(existing_error)` passes through an anyhow error     | No new record              | Preserve the original metadata                 |

The `Result` and `Option` rows refer to anyhow's `Context` extension trait, not
new inherent methods. Cover every existing macro arm, including literal and
formatted messages, typed and boxed errors, and specialized `ensure!` comparison
rendering.

For `.context(...)` on a foreign error or `None`, conversion or creation and
attachment occur at the same call:

- Store one context record on that new layer; do not invent an additional origin
  for the underlying error
- Keep subsequent context calls distinct even when their source locations match

A macro invocation that constructs an error produces one record, not separate
records for its internal helpers.

Preserve the existing dispatch of custom `Into<Error>` implementations. Treat
errors returned through an opaque custom conversion as already constructed: do
not overwrite their entry metadata or claim that the macro observed their
original creation.

## Caller-location rules and limits

Use `#[track_caller]` and [`core::panic::Location::caller()`][location] for
direct function and method boundaries:

- Capture once, before invoking a user-supplied context closure, and pass the
  captured metadata explicitly through private helpers
- Apply tracking to the `Context` trait declarations and audit every constructor
  and dispatch path; annotating `Error::new` alone is insufficient

Follow Rust's [caller-location semantics][track-caller]: a chain of tracked
functions forwards the location from its nearest untracked caller. An untracked
wrapper stops that forwarding.

Function-pointer calls and callbacks such as `.map_err(Error::msg)` do not have
a guaranteed application call site. Retain the compiler-provided hint and
document this limitation rather than guessing a replacement from a backtrace.

Synchronous constructors and context methods called inside an async body can
capture their local call sites. Tracking does not follow task boundaries or
reconstruct an async call stack. Test these calls without relying on an async
function's own `#[track_caller]` annotation.

Rust's [`Result::from_residual`][result-source] calls `From::from`, and its
blanket [`Into` implementation][convert-source] forwards caller information.
These paths can reach the fork's foreign-error conversion. In contrast,
`From<T> for T` returns the same value unchanged, so another `?` on an anyhow
error cannot append a record. Directly returning an existing result has no
interception point either.

### Rich macro metadata

Capture the compiler-reported macro invocation coordinates with `file!`,
`line!`, and `column!`:

- Preserve their normal behavior through nested macros and keep these
  coordinates authoritative even inside a caller annotated with
  `#[track_caller]`
- Store the outer invocation's original argument tokens with `stringify!`, not
  expanded helper tokens
- Do not evaluate arguments again or collect runtime variable values beyond the
  existing message formatting
- Use compiler-reported file paths without filesystem canonicalization

Derive scope at the expansion site using the helper-function type-name technique
in [anyhow-auto-context]:

- Use `core`-compatible helpers without allocating a scope string
- Treat scope as best-effort diagnostic information because
  [`type_name`][type-name] does not guarantee its exact output
- Omit unavailable scope rather than substituting an internal helper name
- Do not fabricate receiver expressions or enclosing function names for ordinary
  methods

## Internal representation

Store location metadata separately from the error and context values. Do not add
synthetic `.context("at ...")` layers, replace typed context with strings, or
wrap arbitrary foreign errors merely to carry a location.

Each newly allocated anyhow layer owns one record containing its event kind
(`created`, `converted`, or `context`), file/line/column, and optional static
macro source text and scope. When adding context, retain inner records and the
original entry rather than replacing them with the most recent attachment.

Extend the private `ErrorImpl` header and, where needed, its vtable to traverse
metadata in anyhow-owned context layers:

- Keep the vtable in its required first position and the erased payload last
- Review layout, alignment, pointer casts, `ManuallyDrop`, owned downcasting,
  and all drop paths together
- Do not reinterpret arbitrary `dyn Error` pointers as anyhow allocation headers
  in the formatter

Keep metadata storage within anyhow's existing allocations:

- Use static references or compact copied descriptors
- Adding a record to an allocation already needed by anyhow must not require
  another heap allocation
- Do not introduce a global registry, thread-local state, a separately allocated
  record vector, or runtime dependencies
- Record storage and traversal should scale with anyhow-owned layers, not stack
  depth

## Diagnostic output

Use the [baseline] formatter in [`src/fmt.rs`](src/fmt.rs). Preserve its report
structure:

- The outer message first, followed by the conditional `Caused by:` heading
- Cause order, with unnumbered single causes and zero-based numbering for
  multiple causes
- Existing message indentation and blank lines
- The optional stack-backtrace section and its presentation

Do not replace the report with a tree, location-first frames, or compiler-style
diagnostics, or introduce a separate `Locations:` section.

Append one ASCII space followed by `[file:line:column]` to the message
associated with each recorded anyhow-owned layer. Do not include `at` or
event-kind labels inside the brackets. Leave messages without associated
metadata unchanged.

For an I/O error first converted in `src/config.rs` and later given context in
`src/main.rs`, render:

```text
loading configuration [src/main.rs:28:10]

Caused by:
    No such file or directory (os error 2) [src/config.rs:12:8]
```

For multiline messages, append annotations to the final message line, before any
trailing line breaks. Apply these preservation rules:

- Preserve all original message characters and the baseline indentation; do not
  trim messages, wrap paths, or add line breaks
- Removing only the added annotations must recover the baseline output for the
  same error messages and backtrace
- With no records, output must match the baseline exactly
- Do not add a new `Error:` prefix inside the formatter

Keep event kinds in the metadata but do not print them. Locations identify
observed creation, conversion, or context-attachment sites, not necessarily
where an underlying failure occurred.

When macro scope or source text is available, append `[scope: ...]` and
`[expression: ...]` after the location on the same message line, in that order.
Escape embedded line breaks and control characters in these optional fields and
omit absent fields.

Do not add continuation lines, causes, or report sections.

Do not add location entries to `chain()` or assign a nearby layer's location to
an opaque foreign source error. Keep repeated formatting read-only and
deterministic for the same metadata.

### Extraction and boxed interoperation

Moving an anyhow error retains its metadata. Successful downcasting into the
original value may discard metadata belonging to the consumed anyhow wrapper.
Do not change downcast behavior to preserve it.

Preserve existing source and boxed-downcast semantics in all cases, and document
metadata loss at these boundaries:

- `into_boxed_dyn_error` keeps the anyhow allocation, so the boxed error's own
  `Debug` formatter can retain its locations
- Re-wrapping that opaque box with `from_boxed` records a new conversion;
  recovering or flattening its earlier metadata is not required
- `reallocate_into_boxed_dyn_error_without_backtrace` may discard metadata with
  the allocation it removes

## Implementation plan

The following steps belong to a subsequent implementation PR. Keep their status
and this specification aligned as implementation proceeds.

1. [ ] Add caller-location regression fixtures for direct constructors, foreign
   `?`/`Into` conversion, context calls, and existing-error propagation before
   changing the representation
2. [ ] Add private records to `src/error.rs` and private metadata traversal;
   cover layout, allocation count, downcasting, and drop behavior
3. [ ] Wire capture through `src/error.rs`, `src/context.rs`, and the `Context`
   declarations in `src/lib.rs`, preserving context closure laziness
4. [ ] Cover `src/macros.rs`, `src/kind.rs`, `__private::format_err`, and both
   specialized and fallback paths in `src/ensure.rs`; add macro metadata without
   changing dispatch, hygiene, evaluation order, or existing-error pass-through
5. [ ] Implement bracketed inline annotations in `src/fmt.rs` and add formatting
   and boxed-interoperation regression tests
6. [ ] Document adoption, supported capture points, and limitations in the
   README; run the compatibility checks below and report measured overhead

## Acceptance and validation

For this specification-only PR, validate the Markdown structure, references, and
diff. Implementation tests and benchmarks are not completion criteria for the
documentation PR. The following checks apply to the implementation PR.

### Capture accuracy and macro coverage

- Assert file, line, and column for each supported boundary, using distinct
  known call sites rather than merely matching a filename
- Exercise every message and error form, macro aliases and nested wrappers,
  generic callers, renamed imports, tracked callers, and specialized and
  fallback `ensure!` dispatch

### Context and existing-error propagation

- Cover `Result` with foreign and anyhow errors, `Option`, `Error::context`,
  generic `Context` bounds, and repeated attachments
- Verify zero context-closure calls on success, one on failure, and panic and
  drop behavior
- Show that existing-error `?`, direct returns, identity conversions, and
  pass-through macros preserve old records without adding new ones

### Async and indirect calls

- Test direct calls inside async bodies, moves between threads and tasks,
  untracked wrappers, function items as callbacks, and function-pointer
  coercions
- Distinguish supported capture from documented hints

### Compatibility and formatting

- Retain upstream tests for cause chains, typed contexts, downcasts, boxed
  conversions, auto traits, layout, macro evaluation, and drop safety
- Update only intentionally changed debug-output expectations
- Assert exact `[file:line:column]` annotations without `at` or event-kind
  labels
- Check `{}`, `{:#}`, and underlying `{:#?}` behavior against the baseline
- Verify that removing annotations recovers baseline `{:?}` output, including
  headings, cause numbering, indentation, blank lines, and backtrace
  presentation
- Cover zero, one, and multiple causes; multiline messages with trailing line
  breaks; optional macro fields; and backtraces disabled and enabled

### Cost

- Compare allocation counts, success-path and error-path timings, allocation
  sizes, and binary size against the baseline
- Verify no added metadata allocations or eager context evaluation
- Report measurements and any toolchain-dependent capture limitations; do not
  claim zero overhead

### CI and additional checks

Run the existing CI coverage: `cargo test`, standard and `--no-default-features`
checks, MSRV builds through `tests/crate/Cargo.toml`, Windows checks, rustdoc,
Clippy, and Miri with the repository's layout/provenance settings. See
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) for the exact commands
and matrix. Add targeted release-mode and MSRV caller-location tests where
ordinary CI does not exercise them.

## Adoption and excluded scope

Keep the package name `anyhow`. Document a workspace-root `[patch.crates-io]`
entry pointing to this fork so compatible direct and transitive dependencies use
the same resolved package.

Follow [Cargo's patch rules][cargo-patch] and verify the resolved graph;
renaming a second package in application imports does not unify its error type
with upstream anyhow.

Do not implement a custom `Result`, custom `Try`, automatic source rewriting,
backtrace-based source reconstruction, or a new public metadata API in this
change. A separately specified opt-in attribute macro could later instrument
`?` in annotated functions, but it would require application edits and must not
be a dependency of the drop-in implementation.

[anyhow-auto-context]: https://github.com/imbolc/anyhow-auto-context/blob/main/src/lib.rs
[baseline]: https://github.com/imbolc/tracked-anyhow/tree/c63b279f3f4af2b02ca6267d9eb47d6d10497f69
[location]: https://doc.rust-lang.org/core/panic/struct.Location.html
[track-caller]: https://doc.rust-lang.org/reference/attributes/codegen.html#the-track_caller-attribute
[result-source]: https://doc.rust-lang.org/src/core/result.rs.html
[convert-source]: https://doc.rust-lang.org/src/core/convert/mod.rs.html
[type-name]: https://doc.rust-lang.org/core/any/fn.type_name.html
[cargo-patch]: https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html
