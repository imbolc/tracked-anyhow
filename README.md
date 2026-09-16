# Tracked anyhow

[<img alt="github" src="https://img.shields.io/badge/github-imbolc/tracked--anyhow-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/imbolc/tracked-anyhow)
[<img alt="crates.io" src="https://img.shields.io/crates/v/tracked-anyhow.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/tracked-anyhow)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-tracked--anyhow-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/tracked-anyhow)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/imbolc/tracked-anyhow/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/imbolc/tracked-anyhow/actions?query=branch%3Amaster)

Adds `[file:line]` to `{:?}` `anyhow` reports. Doesn't require backtraces. Keeps
the fork diff minimal.

```text
loading configuration [src/main.rs:28]

Caused by:
    No such file or directory (os error 2) [src/config.rs:12]
```

You should be able to swap `anyhow` for it without any surprises:

```toml
[dependencies]
anyhow = { package = "tracked-anyhow", version = "0.1.0" }
```

## License

Licensed under either of
<a href="LICENSE-APACHE">Apache License, Version 2.0</a>
or <a href="LICENSE-MIT">MIT license</a> at your option.
