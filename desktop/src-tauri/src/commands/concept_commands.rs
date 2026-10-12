use tauri::State;

use crate::{
    app_error::AppError,
    domain::concept::{
        CardConcept, ConceptDetail, ConceptSummary, SaveConceptAnnotationInput,
        SavedConceptAnnotation,
    },
    repository::concept_repository::ConceptRepository,
    AppState,
};

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn list_concepts(state: State<'_, AppState>) -> Result<Vec<ConceptSummary>, AppError> {
    list_concepts_from_state(&state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn get_concept(
    id: String,
    state: State<'_, AppState>,
) -> Result<ConceptDetail, AppError> {
    get_concept_from_state(id, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn list_card_concepts(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CardConcept>, AppError> {
    list_card_concepts_from_state(card_id, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn delete_concept(id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    delete_concept_from_state(id, &state)
}

#[tauri::command(rename_all = "camelCase")]
pub(crate) fn save_concept_annotation(
    input: SaveConceptAnnotationInput,
    state: State<'_, AppState>,
) -> Result<SavedConceptAnnotation, AppError> {
    save_concept_annotation_from_state(input, &state)
}

fn repository(state: &AppState) -> ConceptRepository<'_> {
    ConceptRepository::new(&state.database)
}

fn list_concepts_from_state(state: &AppState) -> Result<Vec<ConceptSummary>, AppError> {
    repository(state).list_concepts()
}

fn get_concept_from_state(id: String, state: &AppState) -> Result<ConceptDetail, AppError> {
    repository(state).get_concept(&id)
}

fn list_card_concepts_from_state(
    card_id: String,
    state: &AppState,
) -> Result<Vec<CardConcept>, AppError> {
    repository(state).list_card_concepts(&card_id)
}

fn delete_concept_from_state(id: String, state: &AppState) -> Result<(), AppError> {
    repository(state).delete_concept(&id)
}

fn save_concept_annotation_from_state(
    input: SaveConceptAnnotationInput,
    state: &AppState,
) -> Result<SavedConceptAnnotation, AppError> {
    repository(state).save_concept_annotation(input)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        delete_concept_from_state, get_concept_from_state, list_card_concepts_from_state,
        list_concepts_from_state, save_concept_annotation_from_state,
    };
    use crate::{
        database::Database,
        domain::{
            card::{CaptureMethod, NewCardInput, SourceType},
            concept::SaveConceptAnnotationInput,
        },
        repository::{
            attachment_repository::AttachmentRepository, card_repository::CardRepository,
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

    #[test]
    fn concept_commands_complete_the_camel_case_save_and_query_cycle() {
        let (_directory, state) = test_state();
        let card = CardRepository::new(&state.database)
            .create(NewCardInput {
                title: "学习卡片".to_owned(),
                original_content: "费曼学习法".to_owned(),
                note: String::new(),
                source_type: SourceType::Personal,
                capture_method: CaptureMethod::Quick,
                source_name: String::new(),
                source_uri: String::new(),
            })
            .unwrap()
            .card;
        let concept_id = "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01";
        let mention_id = "7fd7d15d-6672-471f-afc8-a8b4cdf1ed0f";

        let saved = save_concept_annotation_from_state(
            SaveConceptAnnotationInput {
                card_id: card.id.clone(),
                concept_id: concept_id.to_owned(),
                mention_id: mention_id.to_owned(),
                selected_text: "费曼学习法".to_owned(),
                name: "费曼学习法".to_owned(),
                explanation: "通过讲解检验理解。".to_owned(),
                overwrite_existing: false,
                document: json!({
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": "费曼学习法",
                            "marks": [{
                                "type": "concept",
                                "attrs": {
                                    "conceptId": concept_id,
                                    "mentionId": mention_id
                                }
                            }]
                        }]
                    }]
                }),
            },
            &state,
        )
        .unwrap();
        let summaries = list_concepts_from_state(&state).unwrap();
        let detail = get_concept_from_state(concept_id.to_owned(), &state).unwrap();
        let card_concepts = list_card_concepts_from_state(card.id.clone(), &state).unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(detail.mentions.len(), 1);
        assert_eq!(card_concepts.len(), 1);
        assert_eq!(
            serde_json::to_value(saved).unwrap(),
            json!({
                "concept": {
                    "id": concept_id,
                    "name": "费曼学习法",
                    "explanation": "通过讲解检验理解。",
                    "mentionCount": 1,
                    "updatedAt": detail.updated_at
                },
                "mention": {
                    "id": concept_id,
                    "name": "费曼学习法",
                    "explanation": "通过讲解检验理解。",
                    "mentionId": mention_id
                },
                "document": {
                    "type": "doc",
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": "费曼学习法",
                            "marks": [{
                                "type": "concept",
                                "attrs": {
                                    "conceptId": concept_id,
                                    "mentionId": mention_id
                                }
                            }]
                        }]
                    }]
                },
                "originalContent": "费曼学习法",
                "updatedAt": detail.updated_at
            })
        );

        delete_concept_from_state(concept_id.to_owned(), &state).unwrap();
        assert!(list_concepts_from_state(&state).unwrap().is_empty());
        assert!(list_card_concepts_from_state(card.id, &state)
            .unwrap()
            .is_empty());
    }
}
