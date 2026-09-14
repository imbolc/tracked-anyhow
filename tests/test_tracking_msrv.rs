#![cfg(all(test, feature = "std"))]

use anyhow::{anyhow, bail, ensure, Error, Result};
use std::error::Error as StdError;
use std::io;

fn foreign() -> io::Error {
    io::Error::new(io::ErrorKind::Other, "foreign")
}

fn assert_location(error: &Error, line: u32) {
    assert_eq!(
        format!("{error:?}").lines().next().unwrap(),
        format!("foreign [{}:{line}]", file!()),
    );
    assert!(error.is::<io::Error>());
}

#[test]
fn foreign_macros_preserve_the_caller() {
    fn bail_foreign(line: &mut u32) -> Result<()> {
        *line = line!() + 1;
        bail!(foreign());
    }

    fn ensure_foreign(line: &mut u32) -> Result<()> {
        *line = line!() + 1;
        ensure!(false, foreign());
        Ok(())
    }

    let line = line!() + 1;
    let error = anyhow!(foreign());
    assert_location(&error, line);

    let mut line = 0;
    assert_location(&bail_foreign(&mut line).unwrap_err(), line);
    assert_location(&ensure_foreign(&mut line).unwrap_err(), line);
}

#[test]
fn generic_foreign_macros_preserve_the_caller() {
    #[track_caller]
    fn convert<E: StdError + Send + Sync + 'static>(error: E) -> Error {
        anyhow!(error)
    }

    let line = line!() + 1;
    let error = convert(foreign());
    assert_location(&error, line);
}

#[test]
fn generic_into_macros_preserve_the_caller() {
    #[track_caller]
    fn convert<E: Into<Error>>(error: E) -> Error {
        anyhow!(error)
    }

    let line = line!() + 1;
    let error = convert(foreign());
    assert_location(&error, line);
}

#[test]
fn direct_conversions_preserve_the_caller() {
    let line = line!() + 1;
    let error = Error::from(foreign());
    assert_location(&error, line);

    let line = line!() + 1;
    let error: Error = foreign().into();
    assert_location(&error, line);
}

#[test]
fn custom_conversions_and_pass_through_keep_the_origin() {
    struct IntoOnly(Error);

    // Exercise an Into implementation with no corresponding From implementation.
    #[allow(clippy::from_over_into)]
    impl Into<Error> for IntoOnly {
        fn into(self) -> Error {
            self.0
        }
    }

    struct FromOnly(Error);

    impl From<FromOnly> for Error {
        fn from(value: FromOnly) -> Self {
            value.0
        }
    }

    fn generic<E: Into<Error>>(error: E) -> Error {
        anyhow!(error)
    }

    let error = Error::msg("original");
    let expected = format!("{error:?}");
    let error = anyhow!(error);
    let error = anyhow!(IntoOnly(error));
    let error = generic(IntoOnly(error));
    let error = anyhow!(FromOnly(error));
    let error = (|| -> Result<()> { bail!(error) })().unwrap_err();
    let error = (|| -> Result<()> {
        ensure!(false, error);
        Ok(())
    })()
    .unwrap_err();
    assert_eq!(format!("{error:?}"), expected);
}
