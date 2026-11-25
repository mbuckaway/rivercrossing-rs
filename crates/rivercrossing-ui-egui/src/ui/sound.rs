// SPDX-License-Identifier: GPL-3.0-only

//! The console's three sound cues and the player that gates them.
//!
//! The cues are embedded at build time from `design/assets/sounds/`, so a
//! missing asset cannot break a launch. A muted player, a machine with no
//! output device and a backend failure are all silent: a cue never blocks the
//! entry field and never raises.

use std::io::Cursor;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink};

#[cfg(test)]
mod tests;

/// The embedded `recorded` cue: a bright tick for a committed crossing.
const RECORDED_WAV: &[u8] = include_bytes!("../../../../design/assets/sounds/recorded.wav");

/// The embedded `flagged` cue: a two-tone chirp for a short lap.
const FLAGGED_WAV: &[u8] = include_bytes!("../../../../design/assets/sounds/flagged.wav");

/// The embedded `error` cue: a low buzz for a refused entry.
const ERROR_WAV: &[u8] = include_bytes!("../../../../design/assets/sounds/error.wav");

/// The sound cue that the console plays for an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cue {
    /// A lap recorded above the minimum.
    Recorded,
    /// A lap recorded but flagged for review.
    Flagged,
    /// A refusal or a miss.
    Error,
}

impl Cue {
    /// Return the stable cue name that the log and the tests use.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Flagged => "flagged",
            Self::Error => "error",
        }
    }

    /// Return the embedded WAV bytes for this cue.
    pub(crate) const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Recorded => RECORDED_WAV,
            Self::Flagged => FLAGGED_WAV,
            Self::Error => ERROR_WAV,
        }
    }
}

/// The playback seam: the rodio device, the silent stand-in or a mock.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SoundBackend {
    /// Play `bytes` for `cue` without blocking the caller.
    ///
    /// # Errors
    ///
    /// Returns the backend's message when the cue cannot be decoded or played.
    fn play(&self, cue: Cue, bytes: &'static [u8]) -> Result<(), String>;
}

/// A backend that accepts every cue and makes no sound.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SilentBackend;

impl SoundBackend for SilentBackend {
    fn play(&self, _cue: Cue, _bytes: &'static [u8]) -> Result<(), String> {
        Ok(())
    }
}

/// The real backend: one shared output stream and one decoder per cue.
#[derive(Debug)]
pub(crate) struct RodioBackend {
    sink: MixerDeviceSink,
}

impl RodioBackend {
    /// Open the default output device, or return `None` when none is usable.
    pub(crate) fn open() -> Option<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
        // The player owns the sink for the whole run, so the dependency's
        // drop notice would only repeat on every exit.
        sink.log_on_drop(false);
        Some(Self { sink })
    }
}

impl SoundBackend for RodioBackend {
    fn play(&self, _cue: Cue, bytes: &'static [u8]) -> Result<(), String> {
        let source = decode(bytes)?;
        self.sink.mixer().add(source);
        Ok(())
    }
}

/// Decode one embedded WAV into a source that the mixer can play.
fn decode(bytes: &'static [u8]) -> Result<Decoder<Cursor<&'static [u8]>>, String> {
    Decoder::try_from(Cursor::new(bytes)).map_err(|error| error.to_string())
}

/// The Settings-gated cue player, holding one backend.
pub(crate) struct SoundPlayer {
    backend: Box<dyn SoundBackend>,
    muted: bool,
}

impl SoundPlayer {
    /// Build an unmuted player over `backend`.
    pub(crate) fn new(backend: Box<dyn SoundBackend>) -> Self {
        Self {
            backend,
            muted: false,
        }
    }

    /// Build a player over the default device, or the silent backend.
    pub(crate) fn with_default_backend() -> Self {
        match RodioBackend::open() {
            Some(backend) => Self::new(Box::new(backend)),
            None => Self::new(Box::new(SilentBackend)),
        }
    }

    /// Mute or unmute the player.
    pub(crate) fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Return whether the player is muted.
    pub(crate) fn is_muted(&self) -> bool {
        self.muted
    }

    /// Play `cue` and return it when the backend accepted it.
    ///
    /// A muted player, an absent device and a backend error all return `None`.
    pub(crate) fn play(&self, cue: Cue) -> Option<Cue> {
        if self.muted {
            return None;
        }
        self.backend.play(cue, cue.bytes()).ok().map(|()| cue)
    }
}
