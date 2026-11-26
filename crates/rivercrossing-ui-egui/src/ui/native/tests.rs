// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the native message boxes: the kind mapping, the accept rule and the headless
//! fallback.

use std::cell::RefCell;
use std::rc::Rc;

use craft::dialogs::{Answer, Message, MessageButtons, MessageLevel, MessageResult, Request, Wake};

use super::{NativeDialogs, accepts};
use crate::ui::action::spec::{CommandId, NativeDialog};
use crate::ui::state::{NativeTask, PendingNative};

/// Return a pending confirmation of *dialog* that quits the app.
fn pending(dialog: NativeDialog) -> PendingNative {
    PendingNative {
        dialog,
        then: NativeTask::Command(CommandId::AppQuit),
    }
}

#[test]
fn test_ask_queues_the_confirmation_and_returns_its_answer() {
    // Arrange
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    let mut native = NativeDialogs::new(Wake::none());
    native.set_override(Some(Box::new(move |request| {
        recorded.borrow_mut().push(request.clone());
        Answer::Message(MessageResult::Ok)
    })));
    native.ask(&pending(NativeDialog::Confirm), "Quit?");

    // Act
    native.show_pending();
    let answers = native.answers();

    // Assert
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].0.dialog, NativeDialog::Confirm);
    assert!(accepts(&answers[0].1));
    let requests = seen.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0],
        Request::Message(Message {
            level: MessageLevel::Info,
            title: "Confirm".to_owned(),
            description: "Quit?".to_owned(),
            buttons: MessageButtons::OkCancel,
        })
    );
}

#[test]
fn test_a_danger_box_is_a_warning_level_and_a_prompt_is_information() {
    // Arrange
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    let mut native = NativeDialogs::new(Wake::none());
    native.set_override(Some(Box::new(move |request| {
        recorded.borrow_mut().push(request.clone());
        Answer::Message(MessageResult::Cancel)
    })));

    // Act
    native.ask(&pending(NativeDialog::Danger), "Stop the ride?");
    native.show_pending();
    let answers = native.answers();
    native.ask(&pending(NativeDialog::Prompt), "Duplicate?");
    native.show_pending();
    let _ = native.answers();

    // Assert
    assert_eq!(answers.len(), 1);
    assert!(!accepts(&answers[0].1));
    let requests = seen.borrow();
    let levels: Vec<MessageLevel> = requests
        .iter()
        .map(|request| match request {
            Request::Message(message) => message.level,
            Request::File(_) => MessageLevel::Info,
        })
        .collect();
    assert_eq!(levels, vec![MessageLevel::Warning, MessageLevel::Info]);
}

#[test]
fn test_an_error_box_offers_one_button() {
    // Arrange
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    let mut native = NativeDialogs::new(Wake::none());
    native.set_override(Some(Box::new(move |request| {
        recorded.borrow_mut().push(request.clone());
        Answer::Message(MessageResult::Ok)
    })));

    // Act
    native.ask(&pending(NativeDialog::Error), "The export failed");
    native.show_pending();
    let _ = native.answers();

    // Assert
    let requests = seen.borrow();
    assert!(matches!(
        requests.first(),
        Some(Request::Message(message))
            if message.level == MessageLevel::Error
                && message.buttons == MessageButtons::Ok
    ));
}

#[test]
fn test_accepts_reads_every_button() {
    // Act and assert
    assert!(accepts(&Answer::Message(MessageResult::Ok)));
    assert!(accepts(&Answer::Message(MessageResult::Yes)));
    assert!(accepts(&Answer::Message(MessageResult::Custom(
        "Open".to_owned()
    ))));
    assert!(!accepts(&Answer::Message(MessageResult::Cancel)));
    assert!(!accepts(&Answer::Message(MessageResult::No)));
    assert!(!accepts(&Answer::Cancelled));
    assert!(!accepts(&Answer::Paths(Vec::new())));
    assert!(!accepts(&Answer::Unavailable("no portal".to_owned())));
}

#[test]
fn test_a_headless_queue_answers_unavailable() {
    // Arrange
    let mut native = NativeDialogs::headless(Wake::none());
    native.ask(&pending(NativeDialog::Confirm), "Quit?");

    // Act
    native.show_pending();
    let answers = native.answers();

    // Assert
    assert_eq!(answers.len(), 1);
    assert!(matches!(&answers[0].1, Answer::Unavailable(reason) if !reason.is_empty()));
    assert!(!accepts(&answers[0].1));
}
