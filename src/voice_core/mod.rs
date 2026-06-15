//! Voice core session and event boundary.
//!
//! This module keeps ASR/audio session orchestration independent from input
//! delivery. The fallback app can adapt these events to SendInput, while a TSF
//! TIP shell can adapt the same events to composition/edit sessions.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::asr::{AsrClient, ResponseType};
use crate::audio::AudioCapture;

pub type SessionId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSource {
    FallbackApp,
    TsfTip,
    TestHarness,
}

#[derive(Debug, Clone)]
pub struct SessionOptions {
    pub source: SessionSource,
    pub language: String,
    pub vad_enabled: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            source: SessionSource::FallbackApp,
            language: "zh-CN".to_string(),
            vad_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingState {
    Recording,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceErrorKind {
    Audio,
    Network,
    Protocol,
    Cancelled,
    Internal,
}

#[derive(Debug, Clone)]
pub enum VoiceEventKind {
    SessionStarted {
        source: SessionSource,
    },
    RecordingStateChanged {
        state: RecordingState,
    },
    InterimText {
        revision: u64,
        text: String,
        is_stable: bool,
    },
    FinalText {
        revision: u64,
        text: String,
    },
    SessionCancelled {
        reason: String,
    },
    SessionEnded,
    Error {
        kind: VoiceErrorKind,
        message: String,
        recoverable: bool,
    },
}

#[derive(Debug, Clone)]
pub struct VoiceEvent {
    pub session_id: SessionId,
    pub kind: VoiceEventKind,
}

pub type VoiceEventReceiver = mpsc::Receiver<VoiceEvent>;

pub struct VoiceCore {
    asr_client: Arc<AsrClient>,
    audio_capture: Arc<AudioCapture>,
    is_recording: Arc<AtomicBool>,
    stop_signal: Arc<AtomicBool>,
    next_session_id: AtomicU64,
}

impl VoiceCore {
    pub fn new(asr_client: Arc<AsrClient>, audio_capture: Arc<AudioCapture>) -> Self {
        Self {
            asr_client,
            audio_capture,
            is_recording: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            next_session_id: AtomicU64::new(1),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub async fn start_session(
        &self,
        options: SessionOptions,
    ) -> Result<(SessionId, VoiceEventReceiver)> {
        if self.is_recording.swap(true, Ordering::SeqCst) {
            return Err(anyhow!("Already recording"));
        }

        let session_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);

        tracing::info!("Starting voice core session {}", session_id);

        let audio_rx = match self.audio_capture.start() {
            Ok(rx) => rx,
            Err(e) => {
                self.is_recording.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };

        let mut result_rx = match self.asr_client.start_realtime(audio_rx).await {
            Ok(rx) => rx,
            Err(e) => {
                self.audio_capture.stop();
                self.is_recording.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };

        let (event_tx, event_rx) = mpsc::channel::<VoiceEvent>(100);
        let is_recording = self.is_recording.clone();
        let stop_signal = self.stop_signal.clone();
        let audio_capture = self.audio_capture.clone();

        tokio::spawn(async move {
            let _ = send_event(
                &event_tx,
                session_id,
                VoiceEventKind::SessionStarted {
                    source: options.source,
                },
            )
            .await;
            let _ = send_event(
                &event_tx,
                session_id,
                VoiceEventKind::RecordingStateChanged {
                    state: RecordingState::Recording,
                },
            )
            .await;

            let mut revision = 0u64;
            let mut ended = false;

            loop {
                if stop_signal.load(Ordering::SeqCst) {
                    tracing::info!("Voice core session {} cancelled by stop signal", session_id);
                    let _ = send_event(
                        &event_tx,
                        session_id,
                        VoiceEventKind::SessionCancelled {
                            reason: "Stopped by user".to_string(),
                        },
                    )
                    .await;
                    break;
                }

                match tokio::time::timeout(std::time::Duration::from_millis(100), result_rx.recv())
                    .await
                {
                    Ok(Some(response)) => match response.response_type {
                        ResponseType::InterimResult => {
                            if !response.text.is_empty() {
                                revision += 1;
                                let _ = send_event(
                                    &event_tx,
                                    session_id,
                                    VoiceEventKind::InterimText {
                                        revision,
                                        text: response.text,
                                        is_stable: false,
                                    },
                                )
                                .await;
                            }
                        }
                        ResponseType::FinalResult => {
                            if !response.text.is_empty() {
                                revision += 1;
                                let _ = send_event(
                                    &event_tx,
                                    session_id,
                                    VoiceEventKind::FinalText {
                                        revision,
                                        text: response.text,
                                    },
                                )
                                .await;
                            }
                        }
                        ResponseType::SessionFinished => {
                            tracing::info!("Voice core session {} finished", session_id);
                            ended = true;
                            let _ = send_event(&event_tx, session_id, VoiceEventKind::SessionEnded)
                                .await;
                            break;
                        }
                        ResponseType::Error => {
                            let _ = send_event(
                                &event_tx,
                                session_id,
                                VoiceEventKind::Error {
                                    kind: VoiceErrorKind::Protocol,
                                    message: response.error_msg,
                                    recoverable: false,
                                },
                            )
                            .await;
                            break;
                        }
                        _ => {}
                    },
                    Ok(None) => {
                        let _ = send_event(
                            &event_tx,
                            session_id,
                            VoiceEventKind::Error {
                                kind: VoiceErrorKind::Network,
                                message: "ASR result channel closed".to_string(),
                                recoverable: false,
                            },
                        )
                        .await;
                        break;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }

            audio_capture.stop();
            is_recording.store(false, Ordering::SeqCst);

            let _ = send_event(
                &event_tx,
                session_id,
                VoiceEventKind::RecordingStateChanged {
                    state: RecordingState::Stopped,
                },
            )
            .await;

            if !ended {
                let _ = send_event(&event_tx, session_id, VoiceEventKind::SessionEnded).await;
            }
        });

        Ok((session_id, event_rx))
    }

    pub async fn stop_session(&self, _session_id: SessionId) -> Result<()> {
        if !self.is_recording() {
            return Ok(());
        }

        self.stop_signal.store(true, Ordering::SeqCst);
        self.audio_capture.stop();
        Ok(())
    }

    pub async fn cancel_session(&self, session_id: SessionId) -> Result<()> {
        self.stop_session(session_id).await
    }
}

async fn send_event(
    event_tx: &mpsc::Sender<VoiceEvent>,
    session_id: SessionId,
    kind: VoiceEventKind,
) -> Result<()> {
    event_tx
        .send(VoiceEvent { session_id, kind })
        .await
        .map_err(|_| anyhow!("Voice event receiver dropped"))
}
