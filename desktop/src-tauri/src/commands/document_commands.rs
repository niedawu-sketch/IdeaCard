use serde_json::Value;
use tauri::{AppHandle, State};

use crate::{
    app_error::AppError,
    repository::{
        card_repository::CreateCardResult,
        document_repository::{
            CardDocument, CreateRichCardInput, DocumentRepository, InlineImageData,
            SavedCardDocument,
        },
    },
    AppState,
};

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn get_card_document(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<CardDocument, AppError> {
    get_card_document_from_state(card_id, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn save_card_document(
    card_id: String,
    document: Value,
    state: State<'_, AppState>,
) -> Result<SavedCardDocument, AppError> {
    save_card_document_from_state(card_id, document, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn load_inline_image(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<InlineImageData, AppError> {
    load_inline_image_from_state(attachment_id, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) async fn create_rich_card(
    input: CreateRichCardInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CreateCardResult, AppError> {
    let mut result = create_rich_card_from_state(input, &state)?;
    result.card = crate::ai::title::initialize_title(app, result.card).await;
    Ok(result)
}

fn repository(state: &AppState) -> DocumentRepository<'_> {
    DocumentRepository::new(&state.database, &state.attachment_repository)
}

fn get_card_document_from_state(
    card_id: String,
    state: &AppState,
) -> Result<CardDocument, AppError> {
    repository(state).get_card_document(&card_id)
}

fn save_card_document_from_state(
    card_id: String,
    document: Value,
    state: &AppState,
) -> Result<SavedCardDocument, AppError> {
    repository(state).save_card_document(&card_id, document)
}

fn load_inline_image_from_state(
    attachment_id: String,
    state: &AppState,
) -> Result<InlineImageData, AppError> {
    repository(state).load_inline_image(&attachment_id)
}

fn create_rich_card_from_state(
    input: CreateRichCardInput,
    state: &AppState,
) -> Result<CreateCardResult, AppError> {
    repository(state).create_rich_card(input)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        create_rich_card_from_state, get_card_document_from_state, load_inline_image_from_state,
        save_card_document_from_state,
    };
    use crate::{
        database::Database,
        domain::card::{CaptureMethod, SourceType},
        repository::{
            attachment_repository::AttachmentRepository, document_repository::CreateRichCardInput,
        },
        AppState,
    };

    fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempdir().unwrap();
        let state = AppState::new(
            Database::open_in_memory().unwrap(),
            AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("cache"),
            ),
        );
        (directory, state)
    }

    fn image_document() -> serde_json::Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "image",
                "attrs": {
                    "tempId": "7b73c44e-cff8-4fb4-97e7-d6a03c3dfd10",
                    "data": "data:image/png;base64,iVBORw0KGgppbmxpbmU=",
                    "alt": "",
                    "title": ""
                }
            }]
        })
    }

    #[test]
    fn command_dtos_use_the_fixed_camel_case_contract() {
        let input = CreateRichCardInput {
            title: "标题".to_owned(),
            note: "想法".to_owned(),
            source_type: SourceType::Web,
            capture_method: CaptureMethod::Clipboard,
            source_name: "来源".to_owned(),
            source_uri: "https://example.com".to_owned(),
            document: json!({ "type": "doc", "content": [] }),
        };

        assert_eq!(
            serde_json::to_value(input).unwrap(),
            json!({
                "title": "标题",
                "note": "想法",
                "sourceType": "web",
                "captureMethod": "clipboard",
                "sourceName": "来源",
                "sourceUri": "https://example.com",
                "document": { "type": "doc", "content": [] }
            })
        );
    }

    #[test]
    fn document_commands_complete_create_get_save_and_load_cycle() {
        let (_directory, state) = test_state();
        let created = create_rich_card_from_state(
            CreateRichCardInput {
                title: String::new(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Clipboard,
                source_name: String::new(),
                source_uri: String::new(),
                document: image_document(),
            },
            &state,
        )
        .unwrap();
        let loaded = get_card_document_from_state(created.card.id.clone(), &state).unwrap();
        let attachment_id = loaded.document["content"][0]["attrs"]["attachmentId"]
            .as_str()
            .unwrap()
            .to_owned();

        let inline = load_inline_image_from_state(attachment_id, &state).unwrap();
        let saved = save_card_document_from_state(
            created.card.id.clone(),
            json!({
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "替换为文字" }]
                }]
            }),
            &state,
        )
        .unwrap();

        assert_eq!(created.card.title, "图片记录");
        assert_eq!(loaded.card_id, created.card.id);
        assert_eq!(inline.mime_type, "image/png");
        assert_eq!(saved.original_content, "替换为文字");
    }
}
