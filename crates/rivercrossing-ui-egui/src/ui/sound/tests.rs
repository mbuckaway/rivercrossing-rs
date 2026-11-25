// SPDX-License-Identifier: GPL-3.0-only

//! Unit tests for the three console cues and the player's mute and error gates.

use mockall::predicate::eq;
use pretty_assertions::assert_eq;
use rodio::Source;

use super::{Cue, MockSoundBackend, SilentBackend, SoundBackend, SoundPlayer, decode};

#[test]
fn test_play_forwards_the_cue_and_its_bytes() {
    // Arrange
    let mut backend = MockSoundBackend::new();
    backend
        .expect_play()
        .with(eq(Cue::Recorded), eq(Cue::Recorded.bytes()))
        .times(1)
        .returning(|_, _| Ok(()));
    let player = SoundPlayer::new(Box::new(backend));

    // Act
    let outcome = player.play(Cue::Recorded);

    // Assert
    assert_eq!(outcome, Some(Cue::Recorded));
}

#[test]
fn test_a_muted_player_is_silent() {
    // Arrange
    let mut backend = MockSoundBackend::new();
    backend
        .expect_play()
        .with(eq(Cue::Error), eq(Cue::Error.bytes()))
        .times(0)
        .returning(|_, _| Ok(()));
    let mut player = SoundPlayer::new(Box::new(backend));
    player.set_muted(true);

    // Act
    let outcome = player.play(Cue::Error);

    // Assert
    assert!(player.is_muted());
    assert_eq!(outcome, None);
}

#[test]
fn test_a_backend_error_is_silent_and_reports_none() {
    // Arrange
    let mut backend = MockSoundBackend::new();
    backend
        .expect_play()
        .with(eq(Cue::Flagged), eq(Cue::Flagged.bytes()))
        .times(1)
        .returning(|_, _| Err("no audio device".to_owned()));
    let player = SoundPlayer::new(Box::new(backend));

    // Act
    let outcome = player.play(Cue::Flagged);

    // Assert
    assert_eq!(outcome, None);
}

#[test]
fn test_the_silent_backend_accepts_any_cue() {
    // Arrange
    let backend = SilentBackend;

    // Act
    let outcome = backend.play(Cue::Flagged, Cue::Flagged.bytes());

    // Assert
    assert_eq!(outcome, Ok(()));
}

#[test]
fn test_the_three_cues_carry_non_empty_wav_bytes() {
    // Arrange
    let recorded = Cue::Recorded.bytes();
    let flagged = Cue::Flagged.bytes();
    let error = Cue::Error.bytes();

    // Act
    let wave_magic = &b"WAVE"[..];

    // Assert
    assert!(recorded.starts_with(b"RIFF"));
    assert!(recorded.len() > 1000);
    assert_eq!(recorded.get(8..12), Some(wave_magic));
    assert!(flagged.starts_with(b"RIFF"));
    assert!(flagged.len() > 1000);
    assert_eq!(flagged.get(8..12), Some(wave_magic));
    assert!(error.starts_with(b"RIFF"));
    assert!(error.len() > 1000);
    assert_eq!(error.get(8..12), Some(wave_magic));
}

#[test]
fn test_the_embedded_cues_decode() {
    // Arrange
    let decoded = decode(Cue::Recorded.bytes()).ok();

    // Act
    let sample_rate = decoded.as_ref().map(|source| source.sample_rate().get());
    let channels = decoded.as_ref().map(|source| source.channels().get());

    // Assert
    assert_eq!(sample_rate, Some(44_100));
    assert_eq!(channels, Some(1));
}

#[test]
fn test_decoding_garbage_bytes_returns_an_error() {
    // Arrange
    let garbage = b"not a wav file";

    // Act
    let decoded = decode(garbage);

    // Assert
    assert!(matches!(decoded, Err(message) if !message.is_empty()));
}

#[test]
fn test_cue_names_are_stable() {
    // Arrange
    let recorded = Cue::Recorded;
    let flagged = Cue::Flagged;
    let error = Cue::Error;

    // Act
    let recorded_name = recorded.as_str();
    let flagged_name = flagged.as_str();
    let error_name = error.as_str();

    // Assert
    assert_eq!(recorded_name, "recorded");
    assert_eq!(flagged_name, "flagged");
    assert_eq!(error_name, "error");
}
