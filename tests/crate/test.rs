#![no_std]

pub use anyhow::*;

// Exercise exported macros and context through the consumer's dependency alias.
pub fn aliased_error() -> Result<()> {
    Err(anyhow::anyhow!("aliased error")).context("package smoke test")
}

#[cfg(test)]
mod tests {
    extern crate std;

    #[test]
    fn alias_uses_tracked_error() {
        let error = super::aliased_error().unwrap_err();
        assert_eq!(std::format!("{}", error), "package smoke test");
        let report = std::format!("{:?}", error);
        assert!(report.starts_with("package smoke test ["));
        assert!(report.contains("\n\nCaused by:\n    aliased error ["));
    }
}
