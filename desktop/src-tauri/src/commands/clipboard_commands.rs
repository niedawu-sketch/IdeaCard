use arboard::{Clipboard, Error as ClipboardError};

use crate::app_error::AppError;

pub trait PlainTextClipboard {
    fn read_text(&mut self) -> Result<Option<String>, AppError>;
    fn write_text(&mut self, text: String) -> Result<(), AppError>;
}

struct SystemTextClipboard {
    clipboard: Clipboard,
}

impl SystemTextClipboard {
    fn open() -> Result<Self, AppError> {
        Clipboard::new()
            .map(|clipboard| Self { clipboard })
            .map_err(|_| clipboard_unavailable())
    }
}

impl PlainTextClipboard for SystemTextClipboard {
    fn read_text(&mut self) -> Result<Option<String>, AppError> {
        match self.clipboard.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(ClipboardError::ContentNotAvailable) => Ok(None),
            Err(_) => Err(clipboard_read_failed()),
        }
    }

    fn write_text(&mut self, text: String) -> Result<(), AppError> {
        self.clipboard
            .set_text(text)
            .map_err(|_| clipboard_write_failed())
    }
}

#[tauri::command]
pub(crate) fn read_clipboard_text() -> Result<Option<String>, AppError> {
    let mut clipboard = SystemTextClipboard::open()?;
    read_clipboard_text_with(&mut clipboard)
}

#[tauri::command]
pub(crate) fn write_clipboard_text(text: String) -> Result<(), AppError> {
    let mut clipboard = SystemTextClipboard::open()?;
    write_clipboard_text_with(&mut clipboard, text)
}

fn read_clipboard_text_with(
    clipboard: &mut impl PlainTextClipboard,
) -> Result<Option<String>, AppError> {
    clipboard.read_text()
}

fn write_clipboard_text_with(
    clipboard: &mut impl PlainTextClipboard,
    text: String,
) -> Result<(), AppError> {
    clipboard.write_text(text)
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

fn clipboard_write_failed() -> AppError {
    AppError::new("clipboard_write_failed", "无法写入剪贴板", "稍后重试")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::app_error::AppError;

    use super::{read_clipboard_text_with, write_clipboard_text_with, PlainTextClipboard};

    struct FakeTextClipboard {
        text: Mutex<Option<String>>,
        read_error: bool,
        write_error: bool,
    }

    impl FakeTextClipboard {
        fn new(text: Option<&str>) -> Self {
            Self {
                text: Mutex::new(text.map(str::to_owned)),
                read_error: false,
                write_error: false,
            }
        }
    }

    impl PlainTextClipboard for FakeTextClipboard {
        fn read_text(&mut self) -> Result<Option<String>, AppError> {
            if self.read_error {
                return Err(AppError::new(
                    "clipboard_read_failed",
                    "无法读取剪贴板内容",
                    "重新复制内容后重试",
                ));
            }
            Ok(self.text.lock().unwrap().clone())
        }

        fn write_text(&mut self, text: String) -> Result<(), AppError> {
            if self.write_error {
                return Err(AppError::new(
                    "clipboard_write_failed",
                    "无法写入剪贴板",
                    "稍后重试",
                ));
            }
            *self.text.lock().unwrap() = Some(text);
            Ok(())
        }
    }

    #[test]
    fn plain_text_commands_read_write_and_preserve_content() {
        let mut clipboard = FakeTextClipboard::new(Some("原文字"));

        assert_eq!(
            read_clipboard_text_with(&mut clipboard).unwrap(),
            Some("原文字".to_owned())
        );
        write_clipboard_text_with(&mut clipboard, "新文字".to_owned()).unwrap();
        assert_eq!(
            read_clipboard_text_with(&mut clipboard).unwrap(),
            Some("新文字".to_owned())
        );
    }

    #[test]
    fn plain_text_read_returns_none_when_clipboard_has_no_text() {
        let mut clipboard = FakeTextClipboard::new(None);

        assert_eq!(read_clipboard_text_with(&mut clipboard).unwrap(), None);
    }

    #[test]
    fn plain_text_commands_keep_stable_error_codes() {
        let mut read_failure = FakeTextClipboard {
            read_error: true,
            ..FakeTextClipboard::new(None)
        };
        let mut write_failure = FakeTextClipboard {
            write_error: true,
            ..FakeTextClipboard::new(None)
        };

        assert_eq!(
            read_clipboard_text_with(&mut read_failure)
                .unwrap_err()
                .code,
            "clipboard_read_failed"
        );
        assert_eq!(
            write_clipboard_text_with(&mut write_failure, "文字".to_owned())
                .unwrap_err()
                .code,
            "clipboard_write_failed"
        );
    }
}
