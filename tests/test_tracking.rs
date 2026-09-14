#![cfg(feature = "std")]
#![allow(
    clippy::assertions_on_result_states,
    clippy::uninlined_format_args,
    clippy::unnecessary_wraps
)]

mod drop;

use self::drop::{DetectDrop, Flag};
use anyhow::{anyhow, bail, ensure, Context, Error, Result};
use futures::FutureExt;
use std::cell::Cell;
use std::error::Error as StdError;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("leaf")]
struct Leaf;

#[derive(Error, Debug)]
#[error("foreign")]
struct Foreign(#[source] Leaf);

fn report(error: &Error) -> String {
    let mut text = format!("{error:?}");
    if let Some(index) = text.find("\n\nStack backtrace:") {
        text.truncate(index);
    }
    text
}

macro_rules! check {
    ($error:expr, $message:expr) => {{
        let error: Error = $error;
        assert_eq!(
            report(&error),
            format!("{} [{}:{}]", $message, file!(), line!()),
        );
    }};
}

macro_rules! check_failure {
    ($expression:expr) => {{
        #[allow(unreachable_code)]
        let result = (|| -> Result<()> {
            $expression;
            Ok(())
        })();
        let error = result.unwrap_err();
        let text = report(&error);
        assert!(text.ends_with(&format!(" [{}:{}]", file!(), line!())), "{text}");
        assert_eq!(error.chain().count(), 1);
    }};
}

#[test]
fn constructors() {
    check!(Error::msg("message"), "message");
    check!(Error::new(Leaf), "leaf");
    check!(Error::from(Leaf), "leaf");
    check!(Leaf.into(), "leaf");
    let boxed: Box<dyn StdError + Send + Sync> = Box::new(Leaf);
    check!(Error::from_boxed(boxed), "leaf");
}

#[test]
fn macro_dispatch() {
    check!(anyhow!("literal"), "literal");
    let value = 7;
    check!(anyhow!("value {value}"), "value 7");
    check!(anyhow!("value {}", value), "value 7");
    let message = String::from("owned");
    check!(anyhow!(message), "owned");
    let message = "borrowed";
    check!(anyhow!(message), "borrowed");
    check!(anyhow!(Leaf), "leaf");
    let boxed: Box<dyn StdError + Send + Sync> = Box::new(Leaf);
    check!(anyhow!(boxed), "leaf");
    check!(anyhow::format_err!("alias"), "alias");
}

#[test]
fn early_return_macros() {
    #[derive(PartialEq)]
    struct NoDebug(u8);

    check_failure!(bail!("literal"));
    check_failure!(bail!("value {}", 7));
    check_failure!(bail!(Leaf));
    check_failure!(ensure!(false));
    check_failure!(ensure!(false, "message"));
    check_failure!(ensure!(false, "value {}", 7));
    check_failure!(ensure!(false, Leaf));
    check_failure!(ensure!(1 == 2));
    check_failure!(ensure!("has spaces" == "other spaces"));
    check_failure!(ensure!(NoDebug(1) == NoDebug(2)));
}

#[test]
fn foreign_question_mark() {
    fn fail() -> std::result::Result<(), Leaf> {
        Err(Leaf)
    }
    fn convert(line: &mut u32) -> Result<()> {
        *line = line!() + 1;
        fail()?;
        Ok(())
    }
    let mut line = 0;
    let error = convert(&mut line).unwrap_err();
    assert_eq!(report(&error), format!("leaf [{}:{line}]", file!()));
    assert!(error.is::<Leaf>());
}

#[test]
fn context_locations_follow_their_own_layers() {
    let root = line!() + 1;
    let error = Error::new(Leaf);
    let middle = line!() + 1;
    let error = error.context("middle");
    let outer = line!() + 1;
    let error = Err::<(), _>(error).with_context(|| "outer").unwrap_err();
    let file = file!();
    assert_eq!(
        report(&error),
        format!("outer [{file}:{outer}]\n\nCaused by:\n    0: middle [{file}:{middle}]\n    1: leaf [{file}:{root}]"),
    );
    assert_eq!(error.to_string(), "outer");
    assert_eq!(format!("{error:#}"), "outer: middle: leaf");
    assert_eq!(error.chain().count(), 3);
    assert!(error.root_cause().is::<Leaf>());
    assert!(error.downcast_ref::<Leaf>().is_some());
    assert!(error.downcast::<Leaf>().is_ok());
}

#[test]
fn foreign_sources_are_not_annotated() {
    let line = line!() + 1;
    let error = Err::<(), _>(Foreign(Leaf)).context("outer").unwrap_err();
    assert_eq!(
        report(&error),
        format!("outer [{}:{line}]\n\nCaused by:\n    0: foreign\n    1: leaf", file!()),
    );
    assert!(error.is::<Foreign>());
}

#[test]
fn foreign_entry_annotates_only_the_converted_error() {
    let line = line!() + 1;
    let error = Error::new(Foreign(Leaf));
    assert_eq!(
        report(&error),
        format!("foreign [{}:{line}]\n\nCaused by:\n    leaf", file!()),
    );
}

#[test]
fn options_and_lazy_context() {
    check!(None::<()>.context("missing").unwrap_err(), "missing");
    check!(None::<()>.with_context(|| "missing").unwrap_err(), "missing");

    let calls = Cell::new(0);
    let context = || {
        calls.set(calls.get() + 1);
        "context"
    };
    assert_eq!(Some(1).with_context(context).unwrap(), 1);
    assert_eq!(Ok::<_, Leaf>(2).with_context(context).unwrap(), 2);
    assert_eq!(calls.get(), 0);
    assert!(None::<()>.with_context(context).is_err());
    assert_eq!(calls.get(), 1);
}

#[test]
fn context_closure_is_called_once_at_the_attachment_site() {
    let calls = Cell::new(0);
    let result = Err::<(), _>(Leaf);
    let line = line!() + 1;
    let result = result.with_context(|| {
        calls.set(calls.get() + 1);
        "context"
    });
    let error = result.unwrap_err();
    assert_eq!(calls.get(), 1);
    assert_eq!(
        report(&error),
        format!("context [{}:{line}]\n\nCaused by:\n    leaf", file!()),
    );
}

#[test]
fn generic_context_dispatch_preserves_the_caller() {
    #[track_caller]
    fn attach<T, E>(result: std::result::Result<T, E>) -> Result<T>
    where
        std::result::Result<T, E>: Context<T, E>,
    {
        result.context("generic")
    }
    let line = line!() + 1;
    let error = attach(Err::<(), _>(Leaf)).unwrap_err();
    assert_eq!(
        report(&error),
        format!("generic [{}:{line}]\n\nCaused by:\n    leaf", file!()),
    );
}

#[test]
fn forwarding_and_pass_through_macros_keep_the_origin() {
    fn forward(result: Result<()>) -> Result<()> {
        result?;
        Ok(())
    }
    fn direct(result: Result<()>) -> Result<()> {
        result
    }
    fn pass_through(error: Error) -> Result<()> {
        bail!(error)
    }
    fn pass_through_ensure(error: Error) -> Result<()> {
        ensure!(false, error);
        Ok(())
    }
    let error = Error::new(Leaf);
    let expected = report(&error);
    let error = forward(direct(Err(error))).unwrap_err();
    let error = anyhow!(error);
    let error = pass_through(error).unwrap_err();
    let error = pass_through_ensure(error).unwrap_err();
    assert_eq!(report(&error), expected);
    let error = std::thread::spawn(move || error).join().unwrap();
    assert_eq!(report(&error), expected);
}

#[test]
fn tracked_wrappers_forward_the_call_site() {
    #[track_caller]
    fn make() -> Error {
        Error::msg("wrapped")
    }
    check!(make(), "wrapped");
}

#[test]
fn indirect_constructors_remain_usable() {
    let constructor: fn(Leaf) -> Error = Error::new;
    assert!(constructor(Leaf).is::<Leaf>());
    let error = Err::<(), _>(Leaf).map_err(Error::new).unwrap_err();
    assert!(error.is::<Leaf>());
}

#[test]
fn boxed_extraction_retains_or_discards_the_existing_allocation() {
    let error = Error::new(Leaf).context("outer");
    let expected = format!("{error:?}");
    let boxed = error.into_boxed_dyn_error();
    assert_eq!(format!("{boxed:?}"), expected);
    assert_eq!(boxed.to_string(), "outer");

    let boxed = Error::new(Leaf).reallocate_into_boxed_dyn_error_without_backtrace();
    assert!(boxed.is::<Leaf>());
    assert_eq!(format!("{boxed:?}"), "Leaf");
}

#[test]
fn rewrapping_an_opaque_box_records_only_the_new_entry() {
    let boxed = Error::new(Leaf).context("outer").into_boxed_dyn_error();
    let line = line!() + 1;
    let error = Error::from_boxed(boxed);
    assert_eq!(
        report(&error),
        format!("outer [{}:{line}]\n\nCaused by:\n    leaf", file!()),
    );
}

#[test]
fn context_reuses_the_original_backtrace() {
    let error = Error::new(Leaf);
    let backtrace: *const _ = error.backtrace();
    let error = error.context("outer");
    assert!(std::ptr::eq(error.backtrace(), backtrace));
}

#[test]
fn multiline_message_bytes_are_preserved() {
    for message in ["", "first\nsecond", "first\n\n", "first\r\n", "[text] "] {
        let line = line!() + 1;
        let error = Error::msg(message);
        let suffix = format!(" [{}:{line}]", file!());
        let text = report(&error);
        assert_eq!(text, format!("{message}{suffix}"));
        assert_eq!(text.strip_suffix(&suffix).unwrap(), message);
    }
}

#[test]
fn multiline_cause_indentation_is_preserved() {
    let root = line!() + 1;
    let error = Error::msg("first\nsecond\n");
    let outer = line!() + 1;
    let error = error.context("outer");
    let plain = report(&error)
        .replace(&format!(" [{}:{outer}]", file!()), "")
        .replace(&format!(" [{}:{root}]", file!()), "");
    assert_eq!(plain, "outer\n\nCaused by:\n    first\n    second\n    ");
}

#[test]
fn async_body_captures_the_local_conversion_site() {
    async fn fail(line: &mut u32) -> Result<()> {
        std::future::ready(()).await;
        *line = line!() + 1;
        Err::<(), _>(Leaf)?;
        Ok(())
    }

    let mut line = 0;
    let error = fail(&mut line).now_or_never().unwrap().unwrap_err();
    assert_eq!(report(&error), format!("leaf [{}:{line}]", file!()));
}

#[derive(Error, Debug)]
#[error("aligned")]
#[repr(align(128))]
struct Aligned(DetectDrop);

#[test]
fn over_aligned_payload_downcasts_and_drops_once() {
    let has_dropped = Flag::new();
    let mut error = Error::new(Aligned(DetectDrop::new(&has_dropped))).context("outer");
    assert!(error.downcast_mut::<Aligned>().is_some());
    let Aligned(value) = error.downcast::<Aligned>().unwrap();
    assert!(!has_dropped.get());
    drop(value);
    assert!(has_dropped.get());
}

#[test]
fn panicking_context_closure_drops_the_error_once() {
    let has_dropped = Flag::new();
    let result = std::panic::catch_unwind(|| {
        let error = Error::new(Aligned(DetectDrop::new(&has_dropped)));
        let _ = Err::<(), _>(error).with_context(|| -> &'static str {
            panic!("context failed")
        });
    });
    assert!(result.is_err());
    assert!(has_dropped.get());
}
