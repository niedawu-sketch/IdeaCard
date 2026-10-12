use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

use crate::app_error::AppError;

pub const MAX_NATIVE_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_SCREENSHOT_NOTE_CHARACTERS: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NativeCommand {
    Ocr,
    Screenshot,
    ScreenPermission,
    KeychainSet,
    KeychainGet,
    KeychainDelete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRequest {
    pub command: NativeCommand,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeResponse {
    pub ok: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub permission: Option<String>,
    #[serde(default)]
    pub capture_kind: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub cancelled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCaptureKind {
    Region,
    Scrolling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeScreenshot {
    pub path: PathBuf,
    pub kind: NativeCaptureKind,
    pub note: String,
}

#[async_trait]
pub trait NativeTransport: Send + Sync {
    async fn request(&self, request: NativeRequest) -> Result<NativeResponse, AppError>;
}

pub struct NativeClient<T> {
    transport: T,
}

impl<T: NativeTransport> NativeClient<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub async fn ocr(&self, path: PathBuf) -> Result<String, AppError> {
        let response = self
            .transport
            .request(NativeRequest {
                command: NativeCommand::Ocr,
                path: Some(serialize_path(&path)?),
                service: None,
                account: None,
                secret: None,
            })
            .await?;
        ensure_native_success(&response)?;

        response.text.ok_or_else(invalid_native_response)
    }

    pub async fn screenshot(
        &self,
        output_path: PathBuf,
    ) -> Result<Option<NativeScreenshot>, AppError> {
        let response = self
            .transport
            .request(NativeRequest {
                command: NativeCommand::Screenshot,
                path: Some(serialize_path(&output_path)?),
                service: None,
                account: None,
                secret: None,
            })
            .await?;
        ensure_native_success(&response)?;

        if response.cancelled == Some(true) {
            if response.text.is_some() || response.path.is_some() || response.capture_kind.is_some()
            {
                return Err(invalid_native_response());
            }
            return Ok(None);
        }

        let path = PathBuf::from(response.path.ok_or_else(invalid_native_response)?);
        if !path.is_absolute() {
            return Err(invalid_native_response());
        }

        let kind = match response.capture_kind.as_deref() {
            Some("region") => NativeCaptureKind::Region,
            Some("scrolling") => NativeCaptureKind::Scrolling,
            _ => return Err(invalid_native_response()),
        };
        let note = response.text.unwrap_or_default();
        if note.chars().count() > MAX_SCREENSHOT_NOTE_CHARACTERS {
            return Err(invalid_native_response());
        }

        Ok(Some(NativeScreenshot { path, kind, note }))
    }

    pub async fn screen_permission(&self) -> Result<String, AppError> {
        let response = self
            .transport
            .request(NativeRequest {
                command: NativeCommand::ScreenPermission,
                path: None,
                service: None,
                account: None,
                secret: None,
            })
            .await?;
        ensure_native_success(&response)?;
        let permission = response.permission.ok_or_else(invalid_native_response)?;
        if !matches!(permission.as_str(), "granted" | "not_granted") {
            return Err(invalid_native_response());
        }
        Ok(permission)
    }
}

pub struct TauriNativeTransport<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriNativeTransport<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

#[async_trait]
impl<R: Runtime> NativeTransport for TauriNativeTransport<R> {
    async fn request(&self, request: NativeRequest) -> Result<NativeResponse, AppError> {
        let mut input = serde_json::to_vec(&request).map_err(|_| native_helper_failed())?;
        input.push(b'\n');

        let command = self
            .app
            .shell()
            .sidecar("ideacard-native")
            .map_err(|_| native_helper_unavailable())?
            .set_raw_out(true);
        let (mut events, mut child) = command.spawn().map_err(|_| native_helper_unavailable())?;
        child.write(&input).map_err(|_| native_helper_failed())?;
        drop(child);

        let mut stdout = Vec::new();
        let mut exit_code = None;
        let mut event_error = false;
        let mut oversized = false;

        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    if !oversized
                        && stdout.len().saturating_add(bytes.len()) <= MAX_NATIVE_OUTPUT_BYTES
                    {
                        stdout.extend_from_slice(&bytes);
                    } else {
                        oversized = true;
                    }
                }
                CommandEvent::Stderr(_) => {}
                CommandEvent::Error(_) => event_error = true,
                CommandEvent::Terminated(payload) => exit_code = payload.code,
                _ => {}
            }
        }

        decode_native_output(exit_code, &stdout, event_error, oversized)
    }
}

pub fn decode_native_output(
    exit_code: Option<i32>,
    stdout: &[u8],
    event_error: bool,
    oversized: bool,
) -> Result<NativeResponse, AppError> {
    if oversized || stdout.len() > MAX_NATIVE_OUTPUT_BYTES {
        return Err(AppError::new(
            "native_response_too_large",
            "原生辅助程序返回的数据过大",
            "重试；问题持续时重新打开应用",
        ));
    }
    if event_error || exit_code != Some(0) {
        return Err(native_helper_failed());
    }

    serde_json::from_slice(stdout).map_err(|_| invalid_native_response())
}

fn serialize_path(path: &Path) -> Result<String, AppError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        AppError::new(
            "native_path_invalid",
            "原生操作使用的文件路径无效",
            "更换文件位置后重试",
        )
    })
}

fn ensure_native_success(response: &NativeResponse) -> Result<(), AppError> {
    if response.ok {
        return Ok(());
    }

    Err(match response.error_code.as_deref() {
        Some("native_file_missing") => AppError::new(
            "native_file_missing",
            "找不到需要处理的文件",
            "重新选择文件后重试",
        ),
        Some("native_image_unsupported") => AppError::new(
            "native_image_unsupported",
            "无法读取这张图片",
            "换用 PNG、JPEG 或 HEIC 图片后重试",
        ),
        Some("native_ocr_failed") => AppError::new(
            "native_ocr_failed",
            "图片文字识别失败",
            "重新截图或更换更清晰的图片后重试",
        ),
        Some("screen_permission_denied") => AppError::new(
            "screen_permission_denied",
            "没有屏幕录制权限",
            "在系统设置中允许 IdeaCard 录制屏幕后重试",
        ),
        Some("screenshot_failed") => AppError::new(
            "screenshot_failed",
            "区域截图失败",
            "重新选择截图区域后重试",
        ),
        Some("scrolling_capture_unreliable") => AppError::new(
            "scrolling_capture_unreliable",
            "页面内容无法可靠拼接，未生成卡片",
            "等待页面停止变化后重试；滚动速度会由 IdeaCard 自动限制",
        ),
        Some("scrolling_target_unavailable") => AppError::new(
            "scrolling_target_unavailable",
            "无法控制目标页面的滚动",
            "把要截图的页面置于最前方后重试",
        ),
        Some("accessibility_not_trusted") => AppError::new(
            "accessibility_not_trusted",
            "滚动截图需要辅助功能权限",
            "在系统设置的辅助功能列表中勾选 IdeaCard 后重新截图",
        ),
        Some("invalid_request") => {
            AppError::new("invalid_request", "原生操作请求无效", "重新打开应用后重试")
        }
        _ => AppError::new(
            "native_operation_failed",
            "macOS 原生操作失败",
            "重试；问题持续时重新打开应用",
        ),
    })
}

fn native_helper_unavailable() -> AppError {
    AppError::new(
        "native_helper_unavailable",
        "无法启动 macOS 原生辅助程序",
        "重新打开应用后重试",
    )
}

fn native_helper_failed() -> AppError {
    AppError::new(
        "native_helper_failed",
        "macOS 原生操作失败",
        "重试；问题持续时重新打开应用",
    )
}

fn invalid_native_response() -> AppError {
    AppError::new(
        "native_response_invalid",
        "原生辅助程序返回了无效结果",
        "重新打开应用后重试",
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::{
        decode_native_output, NativeCaptureKind, NativeClient, NativeCommand, NativeRequest,
        NativeResponse, NativeScreenshot, NativeTransport, MAX_NATIVE_OUTPUT_BYTES,
        MAX_SCREENSHOT_NOTE_CHARACTERS,
    };
    use crate::app_error::AppError;

    struct FakeTransport {
        response: NativeResponse,
        requests: Mutex<Vec<NativeRequest>>,
    }

    impl FakeTransport {
        fn new(response: NativeResponse) -> Self {
            Self {
                response,
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl NativeTransport for FakeTransport {
        async fn request(&self, request: NativeRequest) -> Result<NativeResponse, AppError> {
            self.requests.lock().unwrap().push(request);
            Ok(self.response.clone())
        }
    }

    #[test]
    fn malformed_or_multiple_json_responses_are_rejected() {
        for output in [
            b"not json".as_slice(),
            b"{\"ok\":true}{\"ok\":true}".as_slice(),
        ] {
            let error = decode_native_output(Some(0), output, false, false).unwrap_err();
            assert_eq!(error.code, "native_response_invalid");
        }
    }

    #[test]
    fn sidecar_exit_and_output_overflow_are_stable_errors() {
        let exit_error = decode_native_output(Some(7), b"", false, false).unwrap_err();
        assert_eq!(exit_error.code, "native_helper_failed");

        let oversized = vec![b'x'; MAX_NATIVE_OUTPUT_BYTES + 1];
        let size_error = decode_native_output(Some(0), &oversized, false, true).unwrap_err();
        assert_eq!(size_error.code, "native_response_too_large");
    }

    #[test]
    fn native_error_response_is_mapped_without_exposing_its_message() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: false,
                text: None,
                path: None,
                permission: None,
                capture_kind: None,
                error_code: Some("screen_permission_denied".into()),
                message: Some("SENTINEL private native detail".into()),
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            let error = client.ocr("/tmp/input.png".into()).await.unwrap_err();

            assert_eq!(error.code, "screen_permission_denied");
            assert!(!format!("{error:?}").contains("SENTINEL"));
        });
    }

    #[test]
    fn ocr_preserves_an_empty_successful_result() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: true,
                text: Some(String::new()),
                path: None,
                permission: None,
                capture_kind: None,
                error_code: None,
                message: None,
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            assert_eq!(client.ocr("/tmp/blank.png".into()).await.unwrap(), "");
        });
    }

    #[test]
    fn screenshot_cancellation_returns_no_path() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: true,
                text: None,
                path: None,
                permission: None,
                capture_kind: None,
                error_code: None,
                message: None,
                cancelled: Some(true),
            });
            let client = NativeClient::new(transport);

            assert_eq!(
                client.screenshot("/tmp/capture.png".into()).await.unwrap(),
                None
            );
        });
    }

    #[test]
    fn screenshot_preserves_the_scrolling_capture_kind() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: true,
                text: Some("截图时顺手记下的想法".to_owned()),
                path: Some("/tmp/capture.png".to_owned()),
                permission: None,
                capture_kind: Some("scrolling".to_owned()),
                error_code: None,
                message: None,
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            assert_eq!(
                client.screenshot("/tmp/capture.png".into()).await.unwrap(),
                Some(NativeScreenshot {
                    path: "/tmp/capture.png".into(),
                    kind: NativeCaptureKind::Scrolling,
                    note: "截图时顺手记下的想法".to_owned(),
                })
            );
        });
    }

    #[test]
    fn screenshot_rejects_an_oversized_note() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: true,
                text: Some("想".repeat(MAX_SCREENSHOT_NOTE_CHARACTERS + 1)),
                path: Some("/tmp/capture.png".to_owned()),
                permission: None,
                capture_kind: Some("region".to_owned()),
                error_code: None,
                message: None,
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            let error = client
                .screenshot("/tmp/capture.png".into())
                .await
                .unwrap_err();

            assert_eq!(error.code, "native_response_invalid");
        });
    }

    #[test]
    fn unreliable_scrolling_capture_is_a_stable_failure() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: false,
                text: None,
                path: None,
                permission: None,
                capture_kind: None,
                error_code: Some("scrolling_capture_unreliable".to_owned()),
                message: Some("private matcher detail".to_owned()),
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            let error = client
                .screenshot("/tmp/capture.png".into())
                .await
                .unwrap_err();

            assert_eq!(error.code, "scrolling_capture_unreliable");
            assert_eq!(error.message, "页面内容无法可靠拼接，未生成卡片");
        });
    }

    #[test]
    fn unavailable_scrolling_target_is_not_silently_downgraded() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: false,
                text: None,
                path: None,
                permission: None,
                capture_kind: None,
                error_code: Some("scrolling_target_unavailable".to_owned()),
                message: Some("private window lookup detail".to_owned()),
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            let error = client
                .screenshot("/tmp/capture.png".into())
                .await
                .unwrap_err();

            assert_eq!(error.code, "scrolling_target_unavailable");
            assert_eq!(error.message, "无法控制目标页面的滚动");
            assert_eq!(error.recovery, "把要截图的页面置于最前方后重试");
        });
    }

    #[test]
    fn screen_permission_returns_the_native_status() {
        tauri::async_runtime::block_on(async {
            let transport = FakeTransport::new(NativeResponse {
                ok: true,
                text: None,
                path: None,
                permission: Some("not_granted".to_owned()),
                capture_kind: None,
                error_code: None,
                message: None,
                cancelled: None,
            });
            let client = NativeClient::new(transport);

            assert_eq!(client.screen_permission().await.unwrap(), "not_granted");
        });
    }

    #[test]
    fn request_serialization_matches_the_swift_protocol() {
        let request = NativeRequest {
            command: NativeCommand::ScreenPermission,
            path: None,
            service: None,
            account: None,
            secret: None,
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            serde_json::json!({ "command": "screenPermission" })
        );
    }
}
