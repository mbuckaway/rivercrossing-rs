// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the pinned `ureq` agent policy.

use std::time::Duration;

use pretty_assertions::assert_eq;
use ureq::config::RedirectAuthHeaders;

use super::agent_config;

#[test]
fn test_transport_config_treats_status_as_values() {
    // Arrange
    let config = agent_config();

    // Act
    let status_as_error = config.http_status_as_error();

    // Assert
    assert!(!status_as_error);
}

#[test]
fn test_transport_config_keeps_auth_off_redirects() {
    // Arrange
    let config = agent_config();

    // Act
    let policy = config.redirect_auth_headers();

    // Assert
    assert_eq!(policy, RedirectAuthHeaders::Never);
}

#[test]
fn test_transport_config_sets_the_30_second_timeout() {
    // Arrange
    let config = agent_config();

    // Act
    let global = config.timeouts().global;

    // Assert
    assert_eq!(global, Some(Duration::from_secs(30)));
}
