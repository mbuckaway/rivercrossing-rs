// SPDX-License-Identifier: GPL-3.0-only

//! The native message boxes (R-85) and their answer mapping.
//!
//! Every confirm, danger confirmation, prompt and error report goes through
//! `craft_dialogs::Dialogs`, which queues a message box and answers on a later frame without
//! blocking the event loop. A host that cannot show a box — a headless run, a machine without a
//! desktop portal — answers [`Answer::Unavailable`], and the app then restores the in-app modal, so
//! behaviour is identical headlessly. The nine plan helpers map onto the message kinds:
//! `show_info`/`show_warning`/`show_error` are one-button boxes, `show_confirm`/`show_danger`/
//! `show_prompt` are OK/Cancel boxes, and `show_retry`/`show_ok_open`/`three_choice` are the same
//! builder calls with their own labels when a flow asks for them.

use craft::dialogs::{
    Answer, Dialogs, FileDialog, Message, MessageButtons, MessageLevel, MessageResult, Override,
    Wake,
};

use crate::ui::action::spec::NativeDialog;
use crate::ui::state::{AppState, NativeTask, PendingNative};

/// The app's native message boxes, one open at a time.
pub(crate) struct NativeDialogs {
    /// The queued and open boxes, with the confirmation each one answers.
    dialogs: Dialogs<PendingNative>,
}

impl NativeDialogs {
    /// Build the queue over *wake*, with the real native boxes.
    pub(crate) fn new(wake: Wake) -> Self {
        Self {
            dialogs: Dialogs::new(wake),
        }
    }

    /// Build the queue that answers every box as unavailable.
    ///
    /// A headless host, a snapshot test or the control channel uses this: the app restores its
    /// in-app modal instead of opening a dialog nothing can see.
    pub(crate) fn headless(wake: Wake) -> Self {
        let mut dialogs = Dialogs::new(wake);
        dialogs.set_override(Some(unavailable_override()));
        Self { dialogs }
    }

    /// Queue the box that *pending* names, with *message*.
    pub(crate) fn ask(&mut self, pending: &PendingNative, message: &str) {
        let title = AppState::native_title(pending.dialog);
        self.dialogs
            .ask_message(message_box(pending, title, message), pending.clone());
    }

    /// Queue the save picker that *pending* names.
    pub(crate) fn ask_file(&mut self, pending: &PendingNative, dialog: FileDialog) {
        self.dialogs.ask(dialog, pending.clone());
    }

    /// Show the next queued box at the end of the frame.
    ///
    /// The app has no raw window handle in the UI pass, so the box is shown parentless; a platform
    /// that needs a parent answers [`Answer::Unavailable`] and the app falls back.
    pub(crate) fn show_pending(&mut self) {
        self.dialogs.show_pending(None::<&eframe::Frame>);
    }

    /// Return the answers that arrived since the last call.
    pub(crate) fn answers(&mut self) -> Vec<(PendingNative, Answer)> {
        self.dialogs.take_answers()
    }

    /// Answer every box without a native dialog (the control channel's host).
    pub(crate) fn set_unavailable(&mut self) {
        self.dialogs.set_override(Some(unavailable_override()));
    }

    /// Answer every box through `answer_with` (tests).
    #[cfg(test)]
    pub(crate) fn set_override(&mut self, answer_with: Option<Override>) {
        self.dialogs.set_override(answer_with);
    }
}

impl std::fmt::Debug for NativeDialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeDialogs")
            .field("open", &self.dialogs.is_open())
            .field("queued", &self.dialogs.queued())
            .finish()
    }
}

/// Return whether *answer* accepts the box's question.
#[must_use]
#[cfg(test)]
pub(crate) fn accepts(answer: &Answer) -> bool {
    matches!(
        answer,
        Answer::Message(MessageResult::Ok | MessageResult::Yes | MessageResult::Custom(_))
    )
}

/// Return the task that *answer* runs, when it runs one.
///
/// The publish outcomes carry their own button rules: the success box's `Open` opens the page while
/// its `OK` dismisses, and the failure box's `Retry` retries while its `Cancel` dismisses. Every
/// other box keeps the stock rule.
#[must_use]
#[expect(
    clippy::match_same_arms,
    reason = "each arm spells one button rule; merging the dismissal arms would erase which box they dismiss"
)]
pub(crate) fn accepted_task(pending: &PendingNative, answer: &Answer) -> Option<NativeTask> {
    match (&pending.then, answer) {
        (
            _,
            Answer::Message(MessageResult::Cancel | MessageResult::No)
            | Answer::Cancelled
            | Answer::Paths(_)
            | Answer::Unavailable(_),
        ) => None,
        (NativeTask::OpenPublished { .. }, Answer::Message(MessageResult::Custom(label)))
            if label == "Open" =>
        {
            Some(pending.then.clone())
        }
        (NativeTask::RetryPublish, Answer::Message(MessageResult::Custom(label)))
            if label == "Retry" =>
        {
            Some(pending.then.clone())
        }
        (_, Answer::Message(MessageResult::Custom(_))) => None,
        (NativeTask::OpenPublished { .. }, _) => None,
        (_, Answer::Message(MessageResult::Ok | MessageResult::Yes)) => Some(pending.then.clone()),
    }
}

/// Return the override that answers every request as unavailable.
fn unavailable_override() -> Override {
    Box::new(|_request| Answer::Unavailable("the host shows no native dialogs".to_owned()))
}

/// Build the message box that one pending request asks.
fn message_box(pending: &PendingNative, title: &str, message: &str) -> Message {
    let (level, buttons) = match pending.dialog {
        NativeDialog::Confirm | NativeDialog::Prompt => {
            (MessageLevel::Info, MessageButtons::OkCancel)
        }
        NativeDialog::Danger => (MessageLevel::Warning, MessageButtons::OkCancel),
        NativeDialog::Error => (MessageLevel::Error, MessageButtons::Ok),
    };
    let (level, buttons) = match &pending.then {
        NativeTask::OpenPublished { .. } => (
            level,
            MessageButtons::OkCancelCustom("Open".to_owned(), "OK".to_owned()),
        ),
        NativeTask::RetryPublish => (
            MessageLevel::Error,
            MessageButtons::OkCancelCustom("Retry".to_owned(), "Cancel".to_owned()),
        ),
        NativeTask::ExportFile { .. } => (level, MessageButtons::Ok),
        _ => (level, buttons),
    };
    Message {
        level,
        title: title.to_owned(),
        description: message.to_owned(),
        buttons,
    }
}

#[cfg(test)]
mod tests;
