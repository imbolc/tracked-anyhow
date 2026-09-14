# Automatic error location tracking

Make `tracked-anyhow` a drop-in replacement for `anyhow` that adds only
filename and line annotations to its error diagnostics:

```text
loading configuration [src/main.rs:28]

Caused by:
    No such file or directory (os error 2) [src/config.rs:12]
```

User should not expect surprises doing

```diff
- anyhow = "1.0"
+ anyhow = { package = "tracked-anyhow", version = "0.1" }
```

## Compatibility

- Support Cargo-only migration for code that does not exchange errors with
  upstream `anyhow`; document that aliases do not replace transitive dependencies
  or unify error types
- Keep the public API and type layouts unchanged; preserve error chains, typed
  context, downcasting, and evaluation behavior, including lazy context closures
- Preserve upstream MSRV, feature support, `no_std` support, and backtrace
  behavior
- Keep the code diff from upstream minimal so upstream changes remain easy to
  merge

## Publishing and versioning

Publish the Cargo package as `tracked-anyhow` while retaining the `anyhow`
library target and dependency alias. Use independent fork SemVer with the exact
upstream base in build metadata: `0.1.0+anyhow.1.0.104`. Increment the fork version
for every release, including bug fixes and upstream updates. Never release a
metadata-only version change; Cargo ignores metadata in version requirements.

Keep publication metadata, documentation links, and alias examples consistent.
Validate the aliased downstream test crate and run `cargo publish --dry-run`
before publishing. Do not claim whole-graph replacement or automatic conversion
between upstream and tracked errors.

## Location coverage

- Capture where a message or foreign error becomes an anyhow error through
  constructors, macros, or conversions such as `?` and `Into`
- Capture each explicit context attachment and retain earlier locations
- Direct `context` or `with_context` on a foreign error annotates only the new
  context layer, not the foreign error or its sources; keep this fused operation
  in one allocation
- Moving or forwarding an existing anyhow error, including through `?` or a
  pass-through macro, preserves its locations without adding new ones

Locations identify observed entry or attachment sites, not earlier failures
inside a dependency. Indirect calls and untracked wrappers may obscure the
application call site; document these limitations instead of guessing locations.

Preserve existing extraction and conversion behavior where retaining locations
would change it, and document any metadata loss.

## Diagnostic output

- Add `[file:line]` suffixes only to normal `{:?}` diagnostics, including errors
  returned from `main`; keep other formatting behavior unchanged
- Annotate each message with its own observed location; leave messages without
  captured locations unchanged
- Preserve upstream message text and report structure so removing the added
  annotations recovers the exact original output, including backtraces

## Boundaries

- Tracking is always enabled and works with backtraces disabled
- Capturing locations must not add heap allocations, stack walks, symbolization,
  source-file reads, or runtime dependencies
- Full propagation traces, async call-stack reconstruction, and application
  source rewriting are outside scope
