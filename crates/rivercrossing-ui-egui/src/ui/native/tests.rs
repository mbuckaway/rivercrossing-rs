// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the native message boxes: the kind mapping, the accept rule and the headless
//! fallback.

use std::cell::RefCell;
use std::rc::Rc;

use craft::dialogs::{
    Answer, FileDialog, Message, MessageButtons, MessageLevel, MessageResult, Request, Wake,
};

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

/// Return a pending confirmation that runs *then*.
fn task(then: NativeTask) -> PendingNative {
    PendingNative {
        dialog: crate::ui::action::spec::NativeDialog::Confirm,
        then,
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test is one table of the button rules, kept together so a reader sees them all"
)]
fn test_accepted_task_reads_the_stock_and_publish_buttons() {
    // Arrange
    let quit = task(NativeTask::Command(CommandId::AppQuit));
    let open = task(NativeTask::OpenPublished {
        link: "https://example.test/?page_id=7".to_owned(),
    });
    let retry = task(NativeTask::RetryPublish);

    // Act and assert
    assert_eq!(
        super::accepted_task(&quit, &Answer::Message(MessageResult::Ok)),
        Some(NativeTask::Command(CommandId::AppQuit))
    );
    assert_eq!(
        super::accepted_task(&quit, &Answer::Message(MessageResult::Cancel)),
        None
    );
    assert_eq!(
        super::accepted_task(
            &open,
            &Answer::Message(MessageResult::Custom("Open".to_owned()))
        ),
        Some(NativeTask::OpenPublished {
            link: "https://example.test/?page_id=7".to_owned(),
        })
    );
    assert_eq!(
        super::accepted_task(&open, &Answer::Message(MessageResult::Ok)),
        None
    );
    assert_eq!(
        super::accepted_task(
            &retry,
            &Answer::Message(MessageResult::Custom("Retry".to_owned()))
        ),
        Some(NativeTask::RetryPublish)
    );
    assert_eq!(
        super::accepted_task(
            &retry,
            &Answer::Message(MessageResult::Custom("Cancel".to_owned()))
        ),
        None
    );
    assert_eq!(
        super::accepted_task(&retry, &Answer::Message(MessageResult::Ok)),
        Some(NativeTask::RetryPublish)
    );
    assert_eq!(
        super::accepted_task(
            &task(NativeTask::ExportFile {
                kind: crate::ui::state::exports::ExportKind::Html,
            }),
            &Answer::Paths(vec![std::path::PathBuf::from("/tmp/results.html")]),
        ),
        None
    );
}

#[test]
fn test_ask_file_queues_the_save_picker_and_returns_its_path() {
    // Arrange
    let wanted = std::path::PathBuf::from("/tmp/results.html");
    let answered = wanted.clone();
    let mut native = NativeDialogs::new(Wake::none());
    native.set_override(Some(Box::new(move |_request| {
        Answer::Paths(vec![answered.clone()])
    })));
    let pending = task(NativeTask::ExportFile {
        kind: crate::ui::state::exports::ExportKind::Html,
    });

    // Act
    native.ask_file(&pending, FileDialog::save().file_name("results.html"));
    native.show_pending();
    let answers = native.answers();

    // Assert
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].1, Answer::Paths(vec![wanted]));
    assert!(matches!(
        answers[0].0.then,
        NativeTask::ExportFile {
            kind: crate::ui::state::exports::ExportKind::Html
        }
    ));
}

#[test]
fn test_the_publish_outcome_boxes_carry_their_custom_buttons() {
    // Arrange
    let seen: Rc<RefCell<Vec<Request>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&seen);
    let mut native = NativeDialogs::new(Wake::none());
    native.set_override(Some(Box::new(move |request| {
        recorded.borrow_mut().push(request.clone());
        Answer::Message(MessageResult::Ok)
    })));

    // Act
    native.ask(
        &task(NativeTask::OpenPublished {
            link: "https://example.test".to_owned(),
        }),
        "Published to https://example.test",
    );
    native.show_pending();
    let _ = native.answers();
    native.ask(
        &task(NativeTask::RetryPublish),
        "The publish failed. Retry?",
    );
    native.show_pending();
    let _ = native.answers();

    // Assert
    let requests = seen.borrow();
    assert!(matches!(
        requests.first(),
        Some(Request::Message(message))
            if message.buttons
                == MessageButtons::OkCancelCustom("Open".to_owned(), "OK".to_owned())
    ));
    assert!(matches!(
        requests.get(1),
        Some(Request::Message(message))
            if message.buttons
                == MessageButtons::OkCancelCustom("Retry".to_owned(), "Cancel".to_owned())
    ));
}
