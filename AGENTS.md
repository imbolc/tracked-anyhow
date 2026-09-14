# Development rules

Read `README.md` for behavior and limitations. Follow the versioning convention
in `Cargo.toml`.

- Minimize edits to upstream code and structure. Keep fork-specific additions
  isolated; avoid unrelated refactoring, reformatting, and documentation rewrites.
  Review the cumulative diff against the upstream base, not just the latest commit
- Preserve the public API, public type layouts, evaluation behavior, MSRV,
  features, and `no_std` support. Location capture must not add allocations,
  stack walks, source-file reads, or runtime dependencies
- Preserve upstream CI and tooling unless explicitly asked to change them.
  Reuse existing dependencies and helpers
- Keep tracking-specific regressions in separate files where practical.
  Change upstream test expectations only for intentional behavior differences;
  do not remove useful coverage merely to shrink the diff
- Keep fork-specific documentation concise and primarily in `README.md`.
  Reference existing guidance instead of duplicating it
- Run relevant existing checks and `git diff --check`. Report results and
  checks that could not run
