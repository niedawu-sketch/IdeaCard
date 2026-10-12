use std::{
    fs::{self, OpenOptions},
    io::BufWriter,
    path::{Path, PathBuf},
};

use arboard::{Clipboard, Error as ClipboardError};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};
use url::Url;
use uuid::Uuid;

use crate::{
    app_error::AppError,
    capture::service::CaptureDraft,
    domain::card::SourceType,
    native::client::{NativeClient, NativeTransport},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

pub trait ClipboardReading: Send + Sync {
    fn read_image(&mut self) -> Result<Option<ClipboardImage>, AppError>;
    fn read_text(&mut self) -> Result<Option<String>, AppError>;
}

pub struct SystemClipboard {
    clipboard: Clipboard,
}

impl SystemClipboard {
    pub fn open() -> Result<Self, AppError> {
        Clipboard::new()
            .map(|clipboard| Self { clipboard })
            .map_err(|_| clipboard_unavailable())
    }
}

impl ClipboardReading for SystemClipboard {
    fn read_image(&mut self) -> Result<Option<ClipboardImage>, AppError> {
        match self.clipboard.get_image() {
            Ok(image) => Ok(Some(ClipboardImage {
                width: image.width,
                height: image.height,
                rgba: image.bytes.into_owned(),
            })),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(_) => Err(clipboard_read_failed()),
        }
    }

    fn read_text(&mut self) -> Result<Option<String>, AppError> {
        match self.clipboard.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(_) => Err(clipboard_read_failed()),
        }
    }
}

pub struct ClipboardService<R, T> {
    cache_directory: PathBuf,
    clipboard: R,
    native: NativeClient<T>,
}

impl<R: ClipboardReading, T: NativeTransport> ClipboardService<R, T> {
    pub fn new(cache_directory: PathBuf, clipboard: R, native: NativeClient<T>) -> Self {
        Self {
            cache_directory,
            clipboard,
            native,
        }
    }

    pub fn read_text(&self, text: &str) -> Result<CaptureDraft, AppError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(clipboard_empty());
        }

        let web_url = Url::parse(trimmed)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https"));
        if let Some(url) = web_url {
            return Ok(CaptureDraft::clipboard_text(
                trimmed,
                SourceType::Document,
                url.host_str().unwrap_or_default(),
                trimmed,
            ));
        }

        Ok(CaptureDraft::clipboard_text(
            text,
            SourceType::Personal,
            "",
            "",
        ))
    }

    pub async fn read(&mut self) -> Result<CaptureDraft, AppError> {
        if let Some(image) = self.clipboard.read_image()? {
            return self.read_image(image).await;
        }

        let text = self.clipboard.read_text()?.ok_or_else(clipboard_empty)?;
        self.read_text(&text)
    }

    pub fn temporary_attachment_path(&self, attachment_id: &str) -> Result<PathBuf, AppError> {
        Uuid::parse_str(attachment_id).map_err(|_| temporary_attachment_invalid())?;
        Ok(self.cache_directory.join(format!("{attachment_id}.png")))
    }

    async fn read_image(&self, image: ClipboardImage) -> Result<CaptureDraft, AppError> {
        fs::create_dir_all(&self.cache_directory).map_err(|_| clipboard_image_save_failed())?;

        let attachment_id = Uuid::new_v4().to_string();
        let path = self.temporary_attachment_path(&attachment_id)?;
        write_png(&path, image)?;
        let mut guard = TemporaryImageGuard::new(path.clone());

        let text = self.native.ocr(path).await?;
        let draft = CaptureDraft::clipboard_image(text, attachment_id);
        guard.disarm();
        Ok(draft)
    }
}

fn write_png(path: &Path, image: ClipboardImage) -> Result<(), AppError> {
    let width = u32::try_from(image.width).map_err(|_| clipboard_image_invalid())?;
    let height = u32::try_from(image.height).map_err(|_| clipboard_image_invalid())?;
    let expected_bytes = image
        .width
        .checked_mul(image.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(clipboard_image_invalid)?;
    if width == 0 || height == 0 || image.rgba.len() != expected_bytes {
        return Err(clipboard_image_invalid());
    }

    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| clipboard_image_save_failed())?;
    PngEncoder::new(BufWriter::new(file))
        .write_image(&image.rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|_| clipboard_image_save_failed())
}

struct TemporaryImageGuard {
    path: PathBuf,
    armed: bool,
}

impl TemporaryImageGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryImageGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn clipboard_unavailable() -> AppError {
    AppError::new(
        "clipboard_unavailable",
        "无法访问系统剪贴板",
        "关闭其他正在使用剪贴板的应用后重试",
    )
}

fn clipboard_read_failed() -> AppError {
    AppError::new(
        "clipboard_read_failed",
        "无法读取剪贴板内容",
        "重新复制内容后重试",
    )
}

fn clipboard_empty() -> AppError {
    AppError::new(
        "clipboard_empty",
        "剪贴板中没有可捕获的内容",
        "先复制文字、网址或图片后重试",
    )
}

fn clipboard_image_invalid() -> AppError {
    AppError::new(
        "clipboard_image_invalid",
        "剪贴板图片数据无效",
        "重新复制图片后重试",
    )
}

fn clipboard_image_save_failed() -> AppError {
    AppError::new(
        "clipboard_image_save_failed",
        "无法暂存剪贴板图片",
        "检查磁盘空间后重试",
    )
}

fn temporary_attachment_invalid() -> AppError {
    AppError::new(
        "temporary_attachment_invalid",
        "临时附件标识无效",
        "重新捕获内容后重试",
    )
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Mutex};

    use async_trait::async_trait;
    use image::GenericImageView;
    use tempfile::tempdir;

    use super::{ClipboardImage, ClipboardReading, ClipboardService};
    use crate::{
        app_error::AppError,
        domain::card::{CaptureMethod, SourceType},
        native::client::{NativeClient, NativeRequest, NativeResponse, NativeTransport},
    };

    struct FakeClipboard {
        image: Option<ClipboardImage>,
        text: Option<String>,
    }

    impl ClipboardReading for FakeClipboard {
        fn read_image(&mut self) -> Result<Option<ClipboardImage>, AppError> {
            Ok(self.image.take())
        }

        fn read_text(&mut self) -> Result<Option<String>, AppError> {
            Ok(self.text.take())
        }
    }

    struct FakeNativeTransport {
        result: Mutex<Option<Result<NativeResponse, AppError>>>,
    }

    impl FakeNativeTransport {
        fn success(text: &str) -> Self {
            Self {
                result: Mutex::new(Some(Ok(NativeResponse {
                    ok: true,
                    text: Some(text.to_owned()),
                    path: None,
                    permission: None,
                    capture_kind: None,
                    error_code: None,
                    message: None,
                    cancelled: None,
                }))),
            }
        }

        fn failure() -> Self {
            Self {
                result: Mutex::new(Some(Err(AppError::new(
                    "native_ocr_failed",
                    "图片文字识别失败",
                    "重新截图后重试",
                )))),
            }
        }
    }

    #[async_trait]
    impl NativeTransport for FakeNativeTransport {
        async fn request(&self, _request: NativeRequest) -> Result<NativeResponse, AppError> {
            self.result.lock().unwrap().take().unwrap()
        }
    }

    fn service(
        cache: PathBuf,
        clipboard: FakeClipboard,
        native: FakeNativeTransport,
    ) -> ClipboardService<FakeClipboard, FakeNativeTransport> {
        ClipboardService::new(cache, clipboard, NativeClient::new(native))
    }

    #[test]
    fn classifies_plain_text_and_http_urls() {
        let directory = tempdir().unwrap();
        let service = service(
            directory.path().to_owned(),
            FakeClipboard {
                image: None,
                text: None,
            },
            FakeNativeTransport::success(""),
        );

        let text = service.read_text("临时问题").unwrap();
        assert_eq!(text.kind, CaptureMethod::Clipboard);
        assert_eq!(text.content, "临时问题");
        assert_eq!(text.source_type, SourceType::Personal);
        assert_eq!(text.source_name, "");
        assert_eq!(text.source_uri, "");

        let url = service.read_text("https://example.com/article").unwrap();
        assert_eq!(url.source_type, SourceType::Document);
        assert_eq!(url.source_uri, "https://example.com/article");
        assert_eq!(url.source_name, "example.com");

        let non_http = service.read_text("mailto:hello@example.com").unwrap();
        assert_eq!(non_http.source_type, SourceType::Personal);
        assert_eq!(non_http.source_uri, "");
    }

    #[test]
    fn rejects_empty_text() {
        let directory = tempdir().unwrap();
        let service = service(
            directory.path().to_owned(),
            FakeClipboard {
                image: None,
                text: None,
            },
            FakeNativeTransport::success(""),
        );

        assert_eq!(
            service.read_text("   \n").unwrap_err().code,
            "clipboard_empty"
        );
    }

    #[test]
    fn image_takes_priority_and_becomes_an_ocr_draft_with_a_png() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let clipboard = FakeClipboard {
                image: Some(ClipboardImage {
                    width: 1,
                    height: 1,
                    rgba: vec![23, 42, 99, 255],
                }),
                text: Some("这段文字不应被读取".to_owned()),
            };
            let mut service = service(
                directory.path().to_owned(),
                clipboard,
                FakeNativeTransport::success("识别后的文字"),
            );

            let draft = service.read().await.unwrap();

            assert_eq!(draft.kind, CaptureMethod::Clipboard);
            assert_eq!(draft.content, "识别后的文字");
            assert_eq!(draft.source_type, SourceType::Personal);
            assert_eq!(draft.temporary_attachment_ids.len(), 1);
            let image_path = service
                .temporary_attachment_path(&draft.temporary_attachment_ids[0])
                .unwrap();
            assert!(image_path.is_file());
            assert_eq!(image_path.extension().unwrap(), "png");
            assert_eq!(image::open(image_path).unwrap().dimensions(), (1, 1));
        });
    }

    #[test]
    fn removes_the_temporary_png_when_ocr_fails() {
        tauri::async_runtime::block_on(async {
            let directory = tempdir().unwrap();
            let clipboard = FakeClipboard {
                image: Some(ClipboardImage {
                    width: 1,
                    height: 1,
                    rgba: vec![23, 42, 99, 255],
                }),
                text: None,
            };
            let mut service = service(
                directory.path().to_owned(),
                clipboard,
                FakeNativeTransport::failure(),
            );

            assert_eq!(service.read().await.unwrap_err().code, "native_ocr_failed");
            assert!(directory.path().read_dir().unwrap().next().is_none());
        });
    }
}
