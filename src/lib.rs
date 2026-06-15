//! Doubao Voice Input - Windows voice input tool
//!
//! A lightweight voice input tool that uses Doubao ASR for real-time
//! speech recognition and inserts text into the focused window.

pub mod asr;
pub mod audio;
pub mod business;
pub mod data;
pub mod ui;
pub mod voice_core;

pub use asr::AsrClient;
pub use audio::AudioCapture;
pub use business::{HotkeyManager, TextInserter, VoiceController};
pub use data::{AppConfig, CredentialStore};
pub use voice_core::{
    RecordingState, SessionId, SessionOptions, SessionSource, VoiceCore, VoiceErrorKind,
    VoiceEvent, VoiceEventKind, VoiceEventReceiver,
};
