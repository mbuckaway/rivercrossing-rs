// SPDX-License-Identifier: GPL-3.0-only

//! A recording transport for the unit tests.
//!
//! Each call is recorded, and each scripted answer is popped in order. An
//! empty script answers with an error, so a test that sends one request too
//! many fails with a message instead of a panic.

use std::cell::RefCell;
use std::collections::VecDeque;

use crate::http::{HttpRequest, HttpResponse, Transport};

/// One scripted answer: a status and body, or a transport failure reason.
type Answer = Result<(u16, Vec<u8>), String>;

/// The transport double: replay the scripted answers, record every request.
#[derive(Debug, Default)]
pub(crate) struct MockTransport {
    answers: RefCell<VecDeque<Answer>>,
    calls: RefCell<Vec<HttpRequest>>,
}

impl MockTransport {
    /// Return a transport that answers once with `status` and `body`.
    pub(crate) fn answering(status: u16, body: &str) -> Self {
        Self::answering_bytes(status, body.as_bytes().to_vec())
    }

    /// Return a transport that answers once with the raw `body` bytes.
    pub(crate) fn answering_bytes(status: u16, body: Vec<u8>) -> Self {
        let transport = Self::default();
        transport.answers.borrow_mut().push_back(Ok((status, body)));
        transport
    }

    /// Return a transport whose one answer is the transport failure `reason`.
    pub(crate) fn failing(reason: &str) -> Self {
        let transport = Self::default();
        transport
            .answers
            .borrow_mut()
            .push_back(Err(reason.to_owned()));
        transport
    }

    /// Return the requests that the code under test sent, in order.
    pub(crate) fn calls(&self) -> Vec<HttpRequest> {
        self.calls.borrow().clone()
    }
}

impl Transport for MockTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, String> {
        self.calls.borrow_mut().push(request.clone());
        match self.answers.borrow_mut().pop_front() {
            Some(Ok((status, body))) => Ok(HttpResponse { status, body }),
            Some(Err(reason)) => Err(reason),
            None => Err("mock transport has no scripted response".to_owned()),
        }
    }
}
