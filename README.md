# Tracked anyhow

[<img alt="github" src="https://img.shields.io/badge/github-imbolc/tracked--anyhow-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/imbolc/tracked-anyhow)
[<img alt="crates.io" src="https://img.shields.io/crates/v/tracked-anyhow.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/tracked-anyhow)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-tracked--anyhow-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/tracked-anyhow)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/imbolc/tracked-anyhow/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/imbolc/tracked-anyhow/actions?query=branch%3Amaster)

This fork adds `[file:line]` suffixes to normal `Debug` (`{:?}`) reports while
keeping anyhow's public API, display formats, cause chains, and typed downcasts.
Tracking is always enabled, including when backtraces are disabled.

```text
loading configuration [src/main.rs:28]

Caused by:
    No such file or directory (os error 2) [src/config.rs:12]
```

## Installation

Use the published package through the existing dependency name:

```diff
- anyhow = "1.0"
+ anyhow = { package = "tracked-anyhow", version = "0.1" }
```

```toml
[dependencies]
anyhow = { package = "tracked-anyhow", version = "0.1" }
```

The Cargo package is `tracked-anyhow`; its library target remains `anyhow` so
existing imports, macros, and doctests keep their names. To use this branch
before a release:

```toml
[dependencies.anyhow]
package = "tracked-anyhow"
git = "https://github.com/imbolc/tracked-anyhow"
branch = "tracking-impl"
```

### Dependencies using upstream anyhow

The [dependency alias] changes only the dependency in the adopting crate. It does
not replace transitive dependencies on upstream `anyhow`. Both packages may
coexist, but their `Error` types are distinct, even though both library targets
are named `anyhow`. Update each workspace crate that adopts tracking.

Concrete foreign errors still convert normally. APIs that exchange upstream
`anyhow::Error` or `anyhow::Result` with the fork need explicit conversion or a
coordinated migration. In particular, upstream `anyhow::Error` does not implement
`std::error::Error`, so the fork's blanket conversion does not bridge it through
`?`. A renamed package is not a whole-graph `[patch.crates-io]` replacement.

## Versioning

Versions use independent fork SemVer plus the exact upstream base, for example
`0.1.0+anyhow.1.0.104`. A tracking fix can become `0.1.1+anyhow.1.0.104`; a later
compatible upstream update can become `0.1.2+anyhow.1.0.105`. Choose the fork's
major/minor/patch bump according to compatibility, not the upstream version.

Every release increments the fork version. Never publish versions differing
only in the `+anyhow...` suffix: [Cargo ignores build metadata] in version
requirements. Users request `version = "0.1"`, or a minimum fork version such as
`"0.1.1"` when they need a particular fix. Document upstream updates in release
notes and keep `src/lib.rs`'s documentation root URL in sync with the manifest.

## What locations mean

Constructors, `anyhow!`, `bail!`, `ensure!`, foreign-error conversions, and explicit
context attachments record their observed call sites. Each context keeps the
inner error's earlier location. Passing through an existing anyhow error, including
with `?` or `anyhow!(error)`, does not add a location.

A foreign error's entry location is where it becomes an anyhow error, not where
it originally failed. Its foreign source errors remain unannotated. A direct
`.context(...)` on a foreign error records the attachment site only on the new
context layer.

Caller tracking follows Rust's [`#[track_caller]` forwarding rules]. Untracked
wrappers and indirect calls, including function-pointer calls and callbacks such
as `.map_err(Error::msg)`, can report an internal call site rather than the desired
application call site. Calls inside async bodies can capture their local sites;
this is not an async call-stack or full propagation trace.

Moving an error retains its locations. Successful owned downcasting discards the
consumed anyhow wrappers. `into_boxed_dyn_error()` retains the allocation and its
own debug report; wrapping that opaque box again records a new entry without
recovering the old metadata. Reallocating into the original boxed error type can
discard locations along with the removed allocation.

Tracking stores a static caller-location reference in each existing error
allocation. It adds no allocation solely for capturing a location, no stack walk,
and no runtime dependency. The public `Error` remains one pointer, while its
private allocation is larger. Backtrace collection and formatting are unchanged.

## Validation

Run the existing suite and the added location and allocation regressions with
backtraces disabled for exact diagnostic snapshots:

```sh
RUST_LIB_BACKTRACE=0 cargo test
cargo check --no-default-features
cargo test --manifest-path tests/crate/Cargo.toml
cargo test --manifest-path tests/crate/Cargo.toml --no-default-features
cargo publish --dry-run
```

The publication dry run packages and builds the crate without uploading it.
CI includes that check and the aliased downstream smoke test, alongside MSRV
builds, Windows, Clippy, and Miri.
The upstream documentation below describes the base API; normal debug reports
add the location suffixes described above.

[dependency alias]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#renaming-dependencies-in-cargotoml
[Cargo ignores build metadata]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#version-metadata
[`#[track_caller]` forwarding rules]: https://doc.rust-lang.org/reference/attributes/codegen.html#the-track_caller-attribute

---

## API overview

This library provides [`anyhow::Error`][Error], a trait object based error type
for easy idiomatic error handling in Rust applications.

[Error]: https://docs.rs/tracked-anyhow/0.1/anyhow/struct.Error.html

```toml
[dependencies]
anyhow = { package = "tracked-anyhow", version = "0.1" }
```

<br>

## Details

- Use `Result<T, anyhow::Error>`, or equivalently `anyhow::Result<T>`, as the
  return type of any fallible function.

  Within the function, use `?` to easily propagate any error that implements the
  [`std::error::Error`] trait.

  ```rust
  use anyhow::Result;

  fn get_cluster_info() -> Result<ClusterMap> {
      let config = std::fs::read_to_string("cluster.json")?;
      let map: ClusterMap = serde_json::from_str(&config)?;
      Ok(map)
  }
  ```

  [`std::error::Error`]: https://doc.rust-lang.org/std/error/trait.Error.html

- Attach context to help the person troubleshooting the error understand where
  things went wrong. A low-level error like "No such file or directory" can be
  annoying to debug without more context about what higher level step the
  application was in the middle of.

  ```rust
  use anyhow::{Context, Result};

  fn main() -> Result<()> {
      ...
      it.detach().context("Failed to detach the important thing")?;

      let content = std::fs::read(path)
          .with_context(|| format!("Failed to read instrs from {}", path))?;
      ...
  }
  ```

  ```console
  Error: Failed to read instrs from ./path/to/instrs.json

  Caused by:
      No such file or directory (os error 2)
  ```

- Downcasting is supported and can be by value, by shared reference, or by
  mutable reference as needed.

  ```rust
  // If the error was caused by redaction, then return a
  // tombstone instead of the content.
  match root_cause.downcast_ref::<DataStoreError>() {
      Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
      None => Err(error),
  }
  ```

- If using Rust &ge; 1.65, a backtrace is captured and printed with the error if
  the underlying error type does not already provide its own. In order to see
  backtraces, they must be enabled through the environment variables described
  in [`std::backtrace`]:

  - If you want panics and errors to both have backtraces, set
    `RUST_BACKTRACE=1`;
  - If you want only errors to have backtraces, set `RUST_LIB_BACKTRACE=1`;
  - If you want only panics to have backtraces, set `RUST_BACKTRACE=1` and
    `RUST_LIB_BACKTRACE=0`.

  [`std::backtrace`]: https://doc.rust-lang.org/std/backtrace/index.html#environment-variables

- Anyhow works with any error type that has an impl of `std::error::Error`,
  including ones defined in your crate. We do not bundle a `derive(Error)` macro
  but you can write the impls yourself or use a standalone macro like
  [thiserror].

  ```rust
  use thiserror::Error;

  #[derive(Error, Debug)]
  pub enum FormatError {
      #[error("Invalid header (expected {expected:?}, got {found:?})")]
      InvalidHeader {
          expected: String,
          found: String,
      },
      #[error("Missing attribute: {0}")]
      MissingAttribute(String),
  }
  ```

- One-off error messages can be constructed using the `anyhow!` macro, which
  supports string interpolation and produces an `anyhow::Error`.

  ```rust
  return Err(anyhow!("Missing attribute: {}", missing));
  ```

  A `bail!` macro is provided as a shorthand for the same early return.

  ```rust
  bail!("Missing attribute: {}", missing);
  ```

<br>

## No-std support

In no_std mode, almost all of the same API is available and works the same way.
To depend on Anyhow in no_std mode, disable our default enabled "std" feature in
Cargo.toml. A global allocator is required.

```toml
[dependencies]
anyhow = { package = "tracked-anyhow", version = "0.1", default-features = false }
```

With versions of Rust older than 1.81, no_std mode may require an additional
`.map_err(Error::msg)` when working with a non-Anyhow error type inside a
function that returns Anyhow's error type, as the trait that `?`-based error
conversions are defined by is only available in std in those old versions.

<br>

## Comparison to failure

The `anyhow::Error` type works something like `failure::Error`, but unlike
failure ours is built around the standard library's `std::error::Error` trait
rather than a separate trait `failure::Fail`. The standard library has adopted
the necessary improvements for this to be possible as part of [RFC 2504].

[RFC 2504]: https://github.com/rust-lang/rfcs/blob/master/text/2504-fix-error.md

<br>

## Comparison to thiserror

Use Anyhow if you don't care what error type your functions return, you just
want it to be easy. This is common in application code. Use [thiserror] if you
are a library that wants to design your own dedicated error type(s) so that on
failures the caller gets exactly the information that you choose.

[thiserror]: https://github.com/dtolnay/thiserror

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
