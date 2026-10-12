use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::{app_error::AppError, native, AppState};

const MAX_TARGET_TOKEN_LENGTH: usize = 128;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VoiceInputTargetRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
pub(crate) fn set_voice_input_target(
    target_token: Option<String>,
    target_rect: Option<VoiceInputTargetRect>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let target_token = normalize_target_token(target_token)?;
    #[cfg(debug_assertions)]
    eprintln!(
        "voice-input diagnostic: target_changed available={}",
        target_token.is_some()
    );
    let target_available = target_token.is_some();
    {
        let mut voice_input = state.voice_input.lock().map_err(|_| voice_input_error())?;
        voice_input.set_target(target_token);
    }
    native::voice_input::sync_target(
        &app,
        target_available,
        target_rect.map(|rect| native::voice_input::TargetRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }),
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn finish_voice_input_session(
    session_id: u64,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let finished = {
        let mut voice_input = state.voice_input.lock().map_err(|_| voice_input_error())?;
        voice_input.finish(session_id)
    };
    if finished {
        native::voice_input::end_session(&app, session_id);
    }
    Ok(())
}

fn normalize_target_token(target_token: Option<String>) -> Result<Option<String>, AppError> {
    let Some(target_token) = target_token else {
        return Ok(None);
    };
    let target_token = target_token.trim();
    if target_token.is_empty() || target_token.chars().count() > MAX_TARGET_TOKEN_LENGTH {
        return Err(voice_input_error());
    }
    Ok(Some(target_token.to_owned()))
}

fn voice_input_error() -> AppError {
    AppError::new(
        "voice_input_unavailable",
        "语音输入暂时不可用",
        "重新聚焦输入位置后再试",
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_target_token;

    #[test]
    fn normalizes_optional_target_tokens() {
        assert_eq!(normalize_target_token(None).unwrap(), None);
        assert_eq!(
            normalize_target_token(Some("  target-1  ".to_owned())).unwrap(),
            Some("target-1".to_owned())
        );
    }

    #[test]
    fn rejects_empty_and_oversized_target_tokens() {
        assert!(normalize_target_token(Some("   ".to_owned())).is_err());
        assert!(normalize_target_token(Some("x".repeat(129))).is_err());
    }
}
