//! Voice Controller
//!
//! Fallback app adapter that maps voice core events to SendInput text insertion.

use anyhow::Result;
use std::sync::Arc;

use crate::asr::AsrClient;
use crate::audio::AudioCapture;
use crate::business::TextInserter;
use crate::voice_core::{SessionId, SessionOptions, SessionSource, VoiceCore, VoiceEventKind};

/// Voice input controller
pub struct VoiceController {
    voice_core: Arc<VoiceCore>,
    text_inserter: Arc<TextInserter>,
    active_session: Option<SessionId>,
}

impl VoiceController {
    /// Create a new voice controller
    pub fn new(
        asr_client: Arc<AsrClient>,
        audio_capture: Arc<AudioCapture>,
        text_inserter: Arc<TextInserter>,
    ) -> Self {
        Self {
            voice_core: Arc::new(VoiceCore::new(asr_client, audio_capture)),
            text_inserter,
            active_session: None,
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.voice_core.is_recording()
    }

    /// Toggle voice input on/off
    pub async fn toggle(&mut self) -> Result<()> {
        if self.is_recording() {
            self.stop().await
        } else {
            self.start().await
        }
    }

    /// Start voice input
    pub async fn start(&mut self) -> Result<()> {
        if self.is_recording() {
            return Ok(());
        }

        tracing::info!("Starting fallback voice input...");
        let (session_id, mut event_rx) = self
            .voice_core
            .start_session(SessionOptions {
                source: SessionSource::FallbackApp,
                ..SessionOptions::default()
            })
            .await?;
        self.active_session = Some(session_id);

        let text_inserter = self.text_inserter.clone();

        tokio::spawn(async move {
            let mut last_text = String::new();
            let mut event_count = 0u32;

            tracing::info!("Fallback event adapter started for session {}", session_id);

            while let Some(event) = event_rx.recv().await {
                event_count += 1;
                match event.kind {
                    VoiceEventKind::InterimText { revision, text, .. } => {
                        tracing::debug!("[INTERIM #{}] {}", revision, text);
                        println!("📝 [识别中] {}", text);
                        if let Err(e) = update_text(&text_inserter, &last_text, &text) {
                            tracing::error!("Failed to update text: {}", e);
                        }
                        last_text = text;
                    }
                    VoiceEventKind::FinalText { revision, text } => {
                        tracing::info!("[FINAL #{}] {}", revision, text);
                        println!("✅ [确认] {}", text);
                        if let Err(e) = update_text(&text_inserter, &last_text, &text) {
                            tracing::error!("Failed to update text: {}", e);
                        }
                        // Clear last_text so the next utterance does not delete committed text.
                        last_text.clear();
                    }
                    VoiceEventKind::SessionCancelled { reason } => {
                        tracing::info!("Voice session {} cancelled: {}", session_id, reason);
                        println!("⏹️  [已取消]");
                        break;
                    }
                    VoiceEventKind::SessionEnded => {
                        tracing::info!(
                            "Voice session {} ended after {} events",
                            session_id,
                            event_count
                        );
                        println!("🏁 [会话结束]");
                        break;
                    }
                    VoiceEventKind::Error { message, .. } => {
                        tracing::error!("Voice core error: {}", message);
                        println!("❌ [错误] {}", message);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// Stop voice input
    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_recording() {
            return Ok(());
        }

        tracing::info!("Stopping voice input...");

        if let Some(session_id) = self.active_session.take() {
            self.voice_core.stop_session(session_id).await?;
        }

        // Wait a bit for the task to finish
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        Ok(())
    }
}

/// Update text in the focused window using incremental updates
///
/// Uses prefix matching to minimize deletions and insertions:
/// 1. Find the common prefix between old and new text
/// 2. Only delete characters beyond the common prefix
/// 3. Only append the new suffix
///
/// This significantly reduces visual flickering compared to full replacement.
fn update_text(text_inserter: &TextInserter, old_text: &str, new_text: &str) -> Result<()> {
    // 找到公共前缀长度（无需删除和重新输入的部分）
    let common_prefix_len = old_text
        .chars()
        .zip(new_text.chars())
        .take_while(|(a, b)| a == b)
        .count();

    // 计算需要删除的字符数 = 旧文本超出公共前缀的部分
    let chars_to_delete = old_text.chars().count() - common_prefix_len;

    // 需要追加的文本 = 新文本超出公共前缀的部分
    let text_to_append: String = new_text.chars().skip(common_prefix_len).collect();

    // 执行增量更新
    if chars_to_delete > 0 {
        text_inserter.delete_chars(chars_to_delete)?;
    }
    if !text_to_append.is_empty() {
        text_inserter.insert(&text_to_append)?;
    }

    tracing::debug!(
        "Updated text incrementally: '{}' -> '{}' (kept {} chars, deleted {}, appended '{}')",
        old_text,
        new_text,
        common_prefix_len,
        chars_to_delete,
        text_to_append
    );
    Ok(())
}
