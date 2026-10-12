use tauri::{AppHandle, State};

use crate::{
    app_error::AppError,
    card_lifecycle::CardLifecycleService,
    domain::card::{Card, CardPatch, NewCardInput},
    repository::card_repository::{CardRepository, CreateCardResult},
    AppState,
};

#[tauri::command]
pub(crate) fn list_cards(
    include_archived: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<Card>, AppError> {
    list_cards_from_state(include_archived.unwrap_or(false), &state)
}

#[tauri::command]
pub(crate) fn get_card(id: String, state: State<'_, AppState>) -> Result<Card, AppError> {
    get_card_from_state(id, &state)
}

#[tauri::command]
pub(crate) async fn create_card(
    input: NewCardInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CreateCardResult, AppError> {
    let mut result = create_card_from_state(input, &state)?;
    result.card = crate::ai::title::initialize_title(app, result.card).await;
    Ok(result)
}

#[tauri::command]
pub(crate) fn update_card(
    id: String,
    patch: CardPatch,
    state: State<'_, AppState>,
) -> Result<Card, AppError> {
    update_card_from_state(id, patch, &state)
}

#[tauri::command]
pub(crate) fn archive_card(id: String, state: State<'_, AppState>) -> Result<Card, AppError> {
    archive_card_from_state(id, &state)
}

#[tauri::command]
pub(crate) fn restore_card(id: String, state: State<'_, AppState>) -> Result<Card, AppError> {
    restore_card_from_state(id, &state)
}

#[tauri::command]
pub(crate) fn delete_card(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    delete_card_from_state(id, &state)
}

fn list_cards_from_state(include_archived: bool, state: &AppState) -> Result<Vec<Card>, AppError> {
    CardRepository::new(&state.database).list(include_archived)
}

fn get_card_from_state(id: String, state: &AppState) -> Result<Card, AppError> {
    CardRepository::new(&state.database).get(&id)
}

fn create_card_from_state(
    input: NewCardInput,
    state: &AppState,
) -> Result<CreateCardResult, AppError> {
    CardRepository::new(&state.database).create(input)
}

fn update_card_from_state(
    id: String,
    patch: CardPatch,
    state: &AppState,
) -> Result<Card, AppError> {
    CardRepository::new(&state.database).update(&id, patch)
}

fn archive_card_from_state(id: String, state: &AppState) -> Result<Card, AppError> {
    CardLifecycleService::new(&state.database, &state.attachment_repository).archive(&id)
}

fn restore_card_from_state(id: String, state: &AppState) -> Result<Card, AppError> {
    CardLifecycleService::new(&state.database, &state.attachment_repository).restore(&id)
}

fn delete_card_from_state(id: String, state: &AppState) -> Result<(), AppError> {
    CardLifecycleService::new(&state.database, &state.attachment_repository).delete(&id)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        archive_card_from_state, create_card_from_state, delete_card_from_state,
        get_card_from_state, list_cards_from_state, restore_card_from_state,
        update_card_from_state,
    };
    use crate::{
        database::Database,
        domain::card::{CaptureMethod, CardPatch, CardStatus, NewCardInput, SourceType},
        repository::attachment_repository::AttachmentRepository,
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

    fn new_card() -> NewCardInput {
        NewCardInput {
            title: String::new(),
            original_content: "需要继续分析的问题".to_owned(),
            note: String::new(),
            source_type: SourceType::Personal,
            capture_method: CaptureMethod::Quick,
            source_name: String::new(),
            source_uri: String::new(),
        }
    }

    #[test]
    fn card_commands_delegate_to_the_persistent_repository() {
        let (_directory, state) = test_state();

        let created = create_card_from_state(new_card(), &state).unwrap();
        let loaded = get_card_from_state(created.card.id.clone(), &state).unwrap();
        let updated = update_card_from_state(
            created.card.id.clone(),
            CardPatch {
                note: Some("补充判断".to_owned()),
                ..CardPatch::default()
            },
            &state,
        )
        .unwrap();
        let cards = list_cards_from_state(false, &state).unwrap();

        assert_eq!(loaded, created.card);
        assert_eq!(updated.note, "补充判断");
        assert_eq!(updated.status, CardStatus::Annotated);
        assert_eq!(cards, vec![updated]);
    }

    #[test]
    fn lifecycle_commands_archive_list_restore_and_delete() {
        let (_directory, state) = test_state();
        let created = create_card_from_state(new_card(), &state).unwrap().card;

        let archived = archive_card_from_state(created.id.clone(), &state).unwrap();

        assert_eq!(archived.status, CardStatus::Archived);
        assert!(list_cards_from_state(false, &state).unwrap().is_empty());
        assert_eq!(
            list_cards_from_state(true, &state).unwrap(),
            vec![archived.clone()]
        );

        let restored = restore_card_from_state(archived.id.clone(), &state).unwrap();
        assert_eq!(restored.status, CardStatus::Unprocessed);

        delete_card_from_state(restored.id.clone(), &state).unwrap();
        assert_eq!(
            get_card_from_state(restored.id, &state).unwrap_err().code,
            "card_not_found"
        );
    }

    #[test]
    fn get_card_preserves_the_stable_not_found_error() {
        let (_directory, state) = test_state();

        let error = get_card_from_state("missing".to_owned(), &state).unwrap_err();

        assert_eq!(error.code, "card_not_found");
        assert_eq!(error.message, "找不到这张卡片");
    }
}
