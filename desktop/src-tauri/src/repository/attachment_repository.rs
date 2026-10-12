use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::app_error::AppError;

const DOCUMENT_OR_IMAGE_LIMIT: u64 = 50 * 1024 * 1024;
const MEDIA_LIMIT: u64 = 500 * 1024 * 1024;
const COPY_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentClass {
    DocumentOrImage,
    Media,
}

impl AttachmentClass {
    const fn byte_limit(self) -> u64 {
        match self {
            Self::DocumentOrImage => DOCUMENT_OR_IMAGE_LIMIT,
            Self::Media => MEDIA_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAttachment {
    pub relative_path: String,
    pub byte_size: u64,
    pub checksum: String,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentRepository {
    root: PathBuf,
    cache: PathBuf,
}

pub struct StagedRemoval {
    deleting_root: PathBuf,
    operation_directory: PathBuf,
    moved_files: Vec<(PathBuf, PathBuf)>,
    committed: bool,
}

impl AttachmentRepository {
    pub fn new(root: impl Into<PathBuf>, cache: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cache: cache.into(),
        }
    }

    pub(crate) fn cache_directory(&self) -> &Path {
        &self.cache
    }

    pub fn store(
        &self,
        source: &Path,
        class: AttachmentClass,
    ) -> Result<StoredAttachment, AppError> {
        let metadata = source
            .metadata()
            .map_err(|_| attachment_source_missing_error())?;
        if !metadata.is_file() {
            return Err(attachment_source_missing_error());
        }
        if metadata.len() > class.byte_limit() {
            return Err(attachment_too_large_error(class));
        }
        let extension = validated_extension(source)?;
        let source = File::open(source).map_err(|_| attachment_source_missing_error())?;
        self.store_from_reader(BufReader::new(source), &extension, class)
    }

    pub fn store_bytes(
        &self,
        bytes: &[u8],
        extension: &str,
        class: AttachmentClass,
    ) -> Result<StoredAttachment, AppError> {
        self.store_from_reader(Cursor::new(bytes), extension, class)
    }

    pub fn resolve(&self, relative_path: &str) -> Result<PathBuf, AppError> {
        let relative = Path::new(relative_path);
        let mut component_count = 0;
        for component in relative.components() {
            if !matches!(component, Component::Normal(_)) {
                return Err(attachment_path_invalid_error());
            }
            component_count += 1;
        }
        if component_count == 0 {
            return Err(attachment_path_invalid_error());
        }

        let candidate = self.root.join(relative);
        let metadata = fs::symlink_metadata(&candidate).map_err(|_| attachment_missing_error())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(attachment_path_invalid_error());
        }
        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|_| attachment_missing_error())?;
        let canonical_candidate = candidate
            .canonicalize()
            .map_err(|_| attachment_missing_error())?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(attachment_path_invalid_error());
        }
        Ok(canonical_candidate)
    }

    pub(crate) fn remove_stored(&self, relative_path: &str) -> Result<(), AppError> {
        let path = self.resolve(relative_path)?;
        fs::remove_file(path).map_err(|_| attachment_cleanup_failed_error())
    }

    pub fn stage_removal(&self, relative_paths: &[String]) -> Result<StagedRemoval, AppError> {
        let deleting_root = self.root.join(".deleting");
        let operation_directory = deleting_root.join(Uuid::new_v4().to_string());
        let mut staged = StagedRemoval {
            deleting_root,
            operation_directory,
            moved_files: Vec::with_capacity(relative_paths.len()),
            committed: false,
        };
        if relative_paths.is_empty() {
            return Ok(staged);
        }

        fs::create_dir_all(&staged.operation_directory)
            .map_err(|_| attachment_cleanup_failed_error())?;
        for (index, relative_path) in relative_paths.iter().enumerate() {
            let original = match self.resolve(relative_path) {
                Ok(path) => path,
                Err(error) => {
                    let _ = staged.rollback();
                    return Err(error);
                }
            };
            let file_name = original
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(attachment_path_invalid_error)?;
            let staged_path = staged
                .operation_directory
                .join(format!("{index}-{file_name}"));
            if fs::rename(&original, &staged_path).is_err() {
                let _ = staged.rollback();
                return Err(attachment_cleanup_failed_error());
            }
            staged.moved_files.push((original, staged_path));
        }
        Ok(staged)
    }

    pub fn cleanup_staged_removals(
        &self,
        mut is_referenced: impl FnMut(&str) -> Result<bool, AppError>,
    ) -> Result<(), AppError> {
        let deleting_root = self.root.join(".deleting");
        if !deleting_root.exists() {
            return Ok(());
        }
        for operation in
            fs::read_dir(&deleting_root).map_err(|_| attachment_cleanup_failed_error())?
        {
            let operation = operation.map_err(|_| attachment_cleanup_failed_error())?;
            if !operation
                .file_type()
                .map_err(|_| attachment_cleanup_failed_error())?
                .is_dir()
            {
                fs::remove_file(operation.path()).map_err(|_| attachment_cleanup_failed_error())?;
                continue;
            }
            for staged in
                fs::read_dir(operation.path()).map_err(|_| attachment_cleanup_failed_error())?
            {
                let staged = staged.map_err(|_| attachment_cleanup_failed_error())?;
                let staged_path = staged.path();
                let Some(staged_name) = staged.file_name().to_str().map(str::to_owned) else {
                    fs::remove_file(staged_path).map_err(|_| attachment_cleanup_failed_error())?;
                    continue;
                };
                let Some((index, original_name)) = staged_name.split_once('-') else {
                    fs::remove_file(staged_path).map_err(|_| attachment_cleanup_failed_error())?;
                    continue;
                };
                if index.parse::<usize>().is_err()
                    || validated_relative_file_name(original_name).is_err()
                {
                    fs::remove_file(staged_path).map_err(|_| attachment_cleanup_failed_error())?;
                    continue;
                }
                if is_referenced(original_name)? {
                    let original = self.root.join(original_name);
                    if original.exists() {
                        return Err(attachment_cleanup_failed_error());
                    }
                    fs::rename(staged_path, original)
                        .map_err(|_| attachment_cleanup_failed_error())?;
                } else {
                    fs::remove_file(staged_path).map_err(|_| attachment_cleanup_failed_error())?;
                }
            }
            fs::remove_dir(operation.path()).map_err(|_| attachment_cleanup_failed_error())?;
        }
        fs::remove_dir(deleting_root).map_err(|_| attachment_cleanup_failed_error())
    }

    fn store_from_reader(
        &self,
        mut reader: impl Read,
        extension: &str,
        class: AttachmentClass,
    ) -> Result<StoredAttachment, AppError> {
        let extension = validated_extension_value(extension)?;
        fs::create_dir_all(&self.root).map_err(|_| attachment_copy_failed_error())?;
        fs::create_dir_all(&self.cache).map_err(|_| attachment_copy_failed_error())?;

        let id = Uuid::new_v4().to_string();
        let temp_path = self.cache.join(format!("{id}.tmp"));
        let file_name = format!("{id}.{extension}");
        let final_path = self.root.join(&file_name);
        let temp_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|_| attachment_copy_failed_error())?;
        let mut guard = TempFileGuard::new(temp_path.clone());
        let mut writer = BufWriter::new(temp_file);
        let mut hasher = Sha256::new();
        let mut byte_size = 0_u64;
        let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];

        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|_| attachment_copy_failed_error())?;
            if read == 0 {
                break;
            }
            byte_size = byte_size
                .checked_add(u64::try_from(read).map_err(|_| attachment_copy_failed_error())?)
                .ok_or_else(|| attachment_too_large_error(class))?;
            if byte_size > class.byte_limit() {
                return Err(attachment_too_large_error(class));
            }
            writer
                .write_all(&buffer[..read])
                .map_err(|_| attachment_copy_failed_error())?;
            hasher.update(&buffer[..read]);
        }

        writer.flush().map_err(|_| attachment_copy_failed_error())?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|_| attachment_copy_failed_error())?;
        drop(writer);
        fs::rename(&temp_path, &final_path).map_err(|_| attachment_copy_failed_error())?;
        guard.disarm();

        Ok(StoredAttachment {
            relative_path: file_name,
            byte_size,
            checksum: hex::encode(hasher.finalize()),
            mime_type: mime_guess::from_ext(&extension)
                .first_or_octet_stream()
                .essence_str()
                .to_owned(),
        })
    }
}

fn validated_relative_file_name(value: &str) -> Result<(), AppError> {
    let path = Path::new(value);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(attachment_path_invalid_error());
    }
    Ok(())
}

impl StagedRemoval {
    pub fn rollback(&mut self) -> Result<(), AppError> {
        for (original, staged) in self.moved_files.iter().rev() {
            if let Some(parent) = original.parent() {
                fs::create_dir_all(parent).map_err(|_| attachment_cleanup_failed_error())?;
            }
            fs::rename(staged, original).map_err(|_| attachment_cleanup_failed_error())?;
        }
        self.moved_files.clear();
        self.remove_staging_directories();
        Ok(())
    }

    pub fn commit(mut self) {
        self.committed = true;
        let _ = fs::remove_dir_all(&self.operation_directory);
        self.remove_staging_directories();
    }

    fn remove_staging_directories(&self) {
        let _ = fs::remove_dir(&self.operation_directory);
        let _ = fs::remove_dir(&self.deleting_root);
    }
}

impl Drop for StagedRemoval {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.rollback();
        }
    }
}

#[derive(Debug)]
struct TempFileGuard {
    path: PathBuf,
    armed: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn validated_extension(path: &Path) -> Result<String, AppError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(attachment_extension_invalid_error)?;
    validated_extension_value(extension)
}

fn validated_extension_value(extension: &str) -> Result<String, AppError> {
    let normalized = extension.to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 16
        || !normalized
            .bytes()
            .all(|value| value.is_ascii_alphanumeric())
    {
        return Err(attachment_extension_invalid_error());
    }
    Ok(normalized)
}

fn attachment_source_missing_error() -> AppError {
    AppError::new(
        "attachment_source_missing",
        "找不到要导入的文件",
        "重新选择文件后再试",
    )
}

fn attachment_too_large_error(class: AttachmentClass) -> AppError {
    let limit = match class {
        AttachmentClass::DocumentOrImage => "50 MB",
        AttachmentClass::Media => "500 MB",
    };
    AppError::new(
        "attachment_too_large",
        format!("文件超过 {limit} 限制"),
        "选择更小的文件后再试",
    )
}

fn attachment_extension_invalid_error() -> AppError {
    AppError::new(
        "attachment_extension_invalid",
        "文件扩展名无效",
        "选择支持的文件后再试",
    )
}

fn attachment_copy_failed_error() -> AppError {
    AppError::new(
        "attachment_copy_failed",
        "无法保存附件",
        "检查磁盘空间和数据目录权限后重试",
    )
}

fn attachment_path_invalid_error() -> AppError {
    AppError::new(
        "attachment_path_invalid",
        "附件路径无效",
        "返回卡片后重新导入附件",
    )
}

fn attachment_missing_error() -> AppError {
    AppError::new(
        "attachment_missing",
        "附件文件不存在",
        "保留卡片并重新导入附件",
    )
}

fn attachment_cleanup_failed_error() -> AppError {
    AppError::new(
        "attachment_cleanup_failed",
        "无法清理未完成的附件",
        "重新打开应用后检查附件目录",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Cursor, Read},
        path::Path,
    };

    use tempfile::tempdir;

    use super::{AttachmentClass, AttachmentRepository};

    fn repository(root: &Path) -> AttachmentRepository {
        AttachmentRepository::new(root.join("attachments"), root.join("cache"))
    }

    #[test]
    fn copies_to_temp_then_atomically_moves_and_hashes_file() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("原始资料.txt");
        fs::write(&source, b"hello").unwrap();
        let repository = repository(directory.path());

        let stored = repository
            .store(&source, AttachmentClass::DocumentOrImage)
            .unwrap();

        assert_eq!(stored.byte_size, 5);
        assert_eq!(
            stored.checksum,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(stored.mime_type, "text/plain");
        assert!(stored.relative_path.ends_with(".txt"));
        assert!(!stored.relative_path.contains("原始资料"));
        assert_eq!(
            fs::read(repository.resolve(&stored.relative_path).unwrap()).unwrap(),
            b"hello"
        );
        assert!(fs::read_dir(directory.path().join("cache"))
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn stores_validated_inline_image_bytes_without_a_source_path() {
        let directory = tempdir().unwrap();
        let repository = AttachmentRepository::new(
            directory.path().join("attachments"),
            directory.path().join("cache"),
        );

        let stored = repository
            .store_bytes(
                b"\x89PNG\r\n\x1a\ninline",
                "png",
                AttachmentClass::DocumentOrImage,
            )
            .unwrap();

        assert_eq!(stored.byte_size, 14);
        assert_eq!(stored.mime_type, "image/png");
        assert_eq!(
            fs::read(repository.resolve(&stored.relative_path).unwrap()).unwrap(),
            b"\x89PNG\r\n\x1a\ninline"
        );
        assert!(stored.relative_path.ends_with(".png"));
    }

    #[test]
    fn rejects_document_over_50_megabytes_before_copying() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("large.pdf");
        let file = fs::File::create(&source).unwrap();
        file.set_len(50 * 1024 * 1024 + 1).unwrap();
        let repository = repository(directory.path());

        let error = repository
            .store(&source, AttachmentClass::DocumentOrImage)
            .unwrap_err();

        assert_eq!(error.code, "attachment_too_large");
        assert!(!directory.path().join("attachments").exists());
        assert!(!directory.path().join("cache").exists());
    }

    #[test]
    fn rejects_media_over_500_megabytes_before_copying() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("large.mp4");
        let file = fs::File::create(&source).unwrap();
        file.set_len(500 * 1024 * 1024 + 1).unwrap();
        let repository = repository(directory.path());

        let error = repository
            .store(&source, AttachmentClass::Media)
            .unwrap_err();

        assert_eq!(error.code, "attachment_too_large");
        assert!(!directory.path().join("attachments").exists());
        assert!(!directory.path().join("cache").exists());
    }

    struct FailingReader {
        first: bool,
    }

    impl Read for FailingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.first {
                self.first = false;
                buffer[..5].copy_from_slice(b"hello");
                Ok(5)
            } else {
                Err(io::Error::other("SENTINEL private read failure"))
            }
        }
    }

    #[test]
    fn removes_temp_file_when_copy_fails() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());

        let error = repository
            .store_from_reader(
                FailingReader { first: true },
                "txt",
                AttachmentClass::DocumentOrImage,
            )
            .unwrap_err();

        assert_eq!(error.code, "attachment_copy_failed");
        assert!(!format!("{error:?}").contains("SENTINEL"));
        assert!(fs::read_dir(directory.path().join("cache"))
            .unwrap()
            .next()
            .is_none());
        assert!(fs::read_dir(directory.path().join("attachments"))
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn enforces_stream_limit_even_when_metadata_was_smaller() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        let bytes = vec![0_u8; 50 * 1024 * 1024 + 1];

        let error = repository
            .store_from_reader(Cursor::new(bytes), "pdf", AttachmentClass::DocumentOrImage)
            .unwrap_err();

        assert_eq!(error.code, "attachment_too_large");
        assert!(fs::read_dir(directory.path().join("cache"))
            .unwrap()
            .next()
            .is_none());
        assert!(fs::read_dir(directory.path().join("attachments"))
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn rejects_missing_or_unsafe_extensions() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        for name in ["no-extension", "unsafe.bad-ext", "hidden."] {
            let source = directory.path().join(name);
            fs::write(&source, b"content").unwrap();
            assert_eq!(
                repository
                    .store(&source, AttachmentClass::DocumentOrImage)
                    .unwrap_err()
                    .code,
                "attachment_extension_invalid"
            );
        }
    }

    #[test]
    fn resolve_rejects_unsafe_missing_and_symlink_paths() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        fs::create_dir_all(directory.path().join("attachments")).unwrap();

        for path in [
            "",
            "/tmp/outside.txt",
            "../outside.txt",
            "nested/../../outside",
        ] {
            assert_eq!(
                repository.resolve(path).unwrap_err().code,
                "attachment_path_invalid"
            );
        }
        assert_eq!(
            repository.resolve("missing.txt").unwrap_err().code,
            "attachment_missing"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = directory.path().join("outside.txt");
            fs::write(&outside, b"outside").unwrap();
            symlink(&outside, directory.path().join("attachments/link.txt")).unwrap();
            assert_eq!(
                repository.resolve("link.txt").unwrap_err().code,
                "attachment_path_invalid"
            );
        }
    }

    #[test]
    fn normalizes_uppercase_extensions_without_preserving_source_name() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("Private Name.PDF");
        fs::write(&source, b"pdf").unwrap();
        let repository = repository(directory.path());

        let stored = repository
            .store(&source, AttachmentClass::DocumentOrImage)
            .unwrap();

        assert!(stored.relative_path.ends_with(".pdf"));
        assert!(!stored.relative_path.contains("Private"));
        assert_eq!(stored.mime_type, "application/pdf");
    }

    #[test]
    fn staged_files_are_restored_when_the_guard_is_dropped() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        let attachment = directory.path().join("attachments/card-file.txt");
        fs::create_dir_all(attachment.parent().unwrap()).unwrap();
        fs::write(&attachment, b"content").unwrap();

        {
            let _staged = repository
                .stage_removal(&["card-file.txt".to_owned()])
                .unwrap();
            assert!(!attachment.exists());
        }

        assert_eq!(fs::read(&attachment).unwrap(), b"content");
    }

    #[test]
    fn committed_removal_deletes_original_and_staged_files() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        let attachment = directory.path().join("attachments/card-file.txt");
        fs::create_dir_all(attachment.parent().unwrap()).unwrap();
        fs::write(&attachment, b"content").unwrap();

        repository
            .stage_removal(&["card-file.txt".to_owned()])
            .unwrap()
            .commit();

        assert!(!attachment.exists());
        assert!(!directory.path().join("attachments/.deleting").exists());
    }

    #[test]
    fn startup_cleanup_removes_abandoned_staged_files() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        let abandoned = directory
            .path()
            .join("attachments/.deleting/operation/file.txt");
        fs::create_dir_all(abandoned.parent().unwrap()).unwrap();
        fs::write(&abandoned, b"content").unwrap();

        repository.cleanup_staged_removals(|_| Ok(false)).unwrap();

        assert!(!directory.path().join("attachments/.deleting").exists());
    }

    #[test]
    fn startup_cleanup_restores_a_staged_file_that_is_still_referenced() {
        let directory = tempdir().unwrap();
        let repository = repository(directory.path());
        let staged = directory
            .path()
            .join("attachments/.deleting/operation/0-card-file.txt");
        fs::create_dir_all(staged.parent().unwrap()).unwrap();
        fs::write(&staged, b"content").unwrap();

        repository
            .cleanup_staged_removals(|relative_path| Ok(relative_path == "card-file.txt"))
            .unwrap();

        assert_eq!(
            fs::read(directory.path().join("attachments/card-file.txt")).unwrap(),
            b"content"
        );
        assert!(!directory.path().join("attachments/.deleting").exists());
    }
}
