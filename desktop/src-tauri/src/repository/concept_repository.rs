use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    app_error::AppError,
    database::Database,
    domain::{
        card::CardStatus,
        concept::{
            normalize_concept_name, CardConcept, ConceptDetail, ConceptSource, ConceptSummary,
            SaveConceptAnnotationInput, SavedConceptAnnotation,
        },
        document::{validate_document, DocumentMention},
    },
};

pub struct ConceptRepository<'a> {
    database: &'a Database,
}

impl<'a> ConceptRepository<'a> {
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    pub fn list_concepts(&self) -> Result<Vec<ConceptSummary>, AppError> {
        self.database.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT concepts.id, concepts.name, concepts.explanation,
                        COUNT(concept_mentions.id), concepts.updated_at
                 FROM concepts
                 LEFT JOIN concept_mentions ON concept_mentions.concept_id = concepts.id
                 GROUP BY concepts.id
                 ORDER BY concepts.updated_at DESC, concepts.id ASC",
            )?;
            let concepts = statement
                .query_map([], concept_summary_row)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(concepts)
        })
    }

    pub fn get_concept(&self, id: &str) -> Result<ConceptDetail, AppError> {
        self.database.with_connection(|connection| {
            let summary =
                load_concept_summary(connection, id)?.ok_or_else(concept_not_found_error)?;
            let mut statement = connection.prepare(
                "SELECT concept_mentions.id, cards.id, cards.title,
                        concept_mentions.selected_text, cards.status,
                        concept_mentions.created_at
                 FROM concept_mentions
                 JOIN cards ON cards.id = concept_mentions.card_id
                 WHERE concept_mentions.concept_id = ?1
                 ORDER BY concept_mentions.created_at DESC, concept_mentions.id ASC",
            )?;
            let mentions = statement
                .query_map([id], |row| {
                    let status = row.get::<_, String>(4)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        status,
                        row.get::<_, String>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .map(
                    |(mention_id, card_id, card_title, selected_text, status, created_at)| {
                        Ok(ConceptSource {
                            mention_id,
                            card_id,
                            card_title,
                            selected_text,
                            card_status: CardStatus::from_sql(&status)
                                .ok_or_else(database_data_invalid_error)?,
                            created_at,
                        })
                    },
                )
                .collect::<Result<Vec<_>, AppError>>()?;
            Ok(ConceptDetail {
                id: summary.id,
                name: summary.name,
                explanation: summary.explanation,
                mention_count: summary.mention_count,
                updated_at: summary.updated_at,
                mentions,
            })
        })
    }

    pub fn list_card_concepts(&self, card_id: &str) -> Result<Vec<CardConcept>, AppError> {
        self.database.with_connection(|connection| {
            ensure_card_exists(connection, card_id)?;
            let mut statement = connection.prepare(
                "SELECT concepts.id, concepts.name, concepts.explanation,
                        concept_mentions.id
                 FROM concept_mentions
                 JOIN concepts ON concepts.id = concept_mentions.concept_id
                 WHERE concept_mentions.card_id = ?1
                 ORDER BY concept_mentions.created_at ASC, concept_mentions.id ASC",
            )?;
            let concepts = statement
                .query_map([card_id], |row| {
                    Ok(CardConcept {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        explanation: row.get(2)?,
                        mention_id: row.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(AppError::from)?;
            Ok(concepts)
        })
    }

    pub fn delete_concept(&self, id: &str) -> Result<(), AppError> {
        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let exists = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM concepts WHERE id = ?1)",
                [id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Err(concept_not_found_error());
            }

            let affected_cards = {
                let mut statement = transaction.prepare(
                    "SELECT DISTINCT concept_mentions.card_id, cards.original_content,
                            card_documents.document_json, card_documents.updated_at
                     FROM concept_mentions
                     JOIN cards ON cards.id = concept_mentions.card_id
                     LEFT JOIN card_documents
                       ON card_documents.card_id = concept_mentions.card_id
                     WHERE concept_mentions.concept_id = ?1
                     ORDER BY concept_mentions.card_id ASC",
                )?;
                let rows = statement
                    .query_map([id], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };

            let mut expected_mentions = 0usize;
            for (card_id, original_content, document_json, document_updated_at) in affected_cards {
                let document_json = document_json.ok_or_else(database_data_invalid_error)?;
                let document_updated_at =
                    document_updated_at.ok_or_else(database_data_invalid_error)?;
                let mut document = serde_json::from_str::<Value>(&document_json)
                    .map_err(|_| database_data_invalid_error())?;
                let current = validate_document(&document, false)
                    .map_err(|_| database_data_invalid_error())?;
                if current.plain_text != original_content {
                    return Err(database_data_invalid_error());
                }

                let card_mentions = current
                    .concept_mentions
                    .iter()
                    .filter(|mention| mention.concept_id == id)
                    .count();
                let stored_mentions = transaction.query_row(
                    "SELECT COUNT(*) FROM concept_mentions
                     WHERE card_id = ?1 AND concept_id = ?2",
                    params![card_id, id],
                    |row| row.get::<_, i64>(0),
                )?;
                let stored_mentions =
                    usize::try_from(stored_mentions).map_err(|_| database_data_invalid_error())?;
                if card_mentions == 0 || card_mentions != stored_mentions {
                    return Err(database_data_invalid_error());
                }
                expected_mentions += stored_mentions;

                if remove_concept_marks(&mut document, id)? == 0 {
                    return Err(database_data_invalid_error());
                }
                canonicalize_document(&mut document)?;
                let cleaned = validate_document(&document, false)
                    .map_err(|_| database_data_invalid_error())?;
                if cleaned.plain_text != original_content
                    || cleaned
                        .concept_mentions
                        .iter()
                        .any(|mention| mention.concept_id == id)
                {
                    return Err(database_data_invalid_error());
                }

                let updated_at = monotonic_utc_timestamp(&document_updated_at, Utc::now())?;
                let cleaned_json =
                    serde_json::to_string(&cleaned.document).map_err(|_| concept_delete_error())?;
                let changed = transaction.execute(
                    "UPDATE card_documents
                     SET document_json = ?1, updated_at = ?2
                     WHERE card_id = ?3",
                    params![cleaned_json, updated_at, card_id],
                )?;
                if changed != 1 {
                    return Err(concept_delete_error());
                }
            }

            let removed_mentions =
                transaction.execute("DELETE FROM concept_mentions WHERE concept_id = ?1", [id])?;
            if removed_mentions != expected_mentions {
                return Err(database_data_invalid_error());
            }
            let removed_concept =
                transaction.execute("DELETE FROM concepts WHERE id = ?1", [id])?;
            if removed_concept != 1 {
                return Err(concept_delete_error());
            }
            transaction.commit()?;
            Ok(())
        })
    }

    pub fn save_concept_annotation(
        &self,
        input: SaveConceptAnnotationInput,
    ) -> Result<SavedConceptAnnotation, AppError> {
        validate_annotation_input(&input)?;
        let validated = validate_document(&input.document, false)
            .map_err(|_| concept_selection_stale_error())?;
        let target = validated
            .concept_mentions
            .iter()
            .find(|mention| mention.mention_id == input.mention_id)
            .filter(|mention| {
                mention.concept_id == input.concept_id
                    && mention.selected_text == input.selected_text
            })
            .ok_or_else(concept_selection_stale_error)?;
        if validated
            .concept_mentions
            .iter()
            .filter(|mention| mention.mention_id == input.mention_id)
            .count()
            != 1
            || target.selected_text.trim().is_empty()
        {
            return Err(concept_selection_stale_error());
        }

        self.database.with_connection(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let (status, previous_content, previous_updated_at, current_document) =
                load_card_annotation_state(&transaction, &input.card_id)?;
            if status == CardStatus::Archived {
                return Err(card_archived_read_only_error());
            }
            validate_current_document(
                &input.document,
                &input.mention_id,
                &previous_content,
                current_document.as_deref(),
                input.overwrite_existing,
            )?;
            validate_existing_images(&transaction, &input.card_id, &validated.persisted_image_ids)?;
            validate_existing_mentions(
                &transaction,
                &input.card_id,
                &validated.concept_mentions,
                &input.mention_id,
            )?;

            let normalized_name = normalize_concept_name(&input.name)?;
            let display_name = display_concept_name(&input.name)?;
            let explanation = input.explanation.trim().to_owned();
            if input.overwrite_existing {
                let existing = load_concept_summary(&transaction, &input.concept_id)?
                    .ok_or_else(concept_selection_stale_error)?;
                if normalize_concept_name(&existing.name)? != normalized_name {
                    return Err(concept_selection_stale_error());
                }
                let target_exists = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM concept_mentions
                        WHERE id = ?1 AND card_id = ?2 AND concept_id = ?3
                          AND selected_text = ?4
                     )",
                    params![
                        input.mention_id,
                        input.card_id,
                        input.concept_id,
                        input.selected_text
                    ],
                    |row| row.get::<_, bool>(0),
                )?;
                if !target_exists {
                    return Err(concept_selection_stale_error());
                }
                let updated_at = monotonic_utc_timestamp(&existing.updated_at, Utc::now())?;
                let changed = transaction.execute(
                    "UPDATE concepts SET explanation = ?1, updated_at = ?2 WHERE id = ?3",
                    params![explanation, updated_at, input.concept_id],
                )?;
                if changed != 1 {
                    return Err(concept_save_error());
                }
                let concept = load_concept_summary(&transaction, &input.concept_id)?
                    .ok_or_else(concept_save_error)?;
                let mention = CardConcept {
                    id: concept.id.clone(),
                    name: concept.name.clone(),
                    explanation: concept.explanation.clone(),
                    mention_id: input.mention_id.clone(),
                };
                transaction.commit()?;
                return Ok(SavedConceptAnnotation {
                    concept,
                    mention,
                    document: validated.document.clone(),
                    original_content: validated.plain_text.clone(),
                    updated_at,
                });
            }
            let updated_at = monotonic_utc_timestamp(&previous_updated_at, Utc::now())?;
            let existing = load_concept_by_normalized_name(&transaction, &normalized_name)?;
            let actual_concept_id = if let Some(summary) = existing {
                summary.id
            } else {
                if transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM concepts WHERE id = ?1)",
                    [&input.concept_id],
                    |row| row.get::<_, bool>(0),
                )? {
                    return Err(concept_selection_stale_error());
                }
                transaction.execute(
                    "INSERT INTO concepts (
                        id, name, normalized_name, explanation, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![
                        input.concept_id,
                        display_name,
                        normalized_name,
                        explanation,
                        updated_at
                    ],
                )?;
                input.concept_id.clone()
            };

            let mut persisted_document = input.document.clone();
            replace_target_concept_id(
                &mut persisted_document,
                &input.mention_id,
                &input.concept_id,
                &actual_concept_id,
            )?;
            let persisted = validate_document(&persisted_document, false)
                .map_err(|_| concept_selection_stale_error())?;
            let persisted_target = persisted
                .concept_mentions
                .iter()
                .find(|mention| mention.mention_id == input.mention_id)
                .filter(|mention| {
                    mention.concept_id == actual_concept_id
                        && mention.selected_text == input.selected_text
                })
                .ok_or_else(concept_selection_stale_error)?;

            transaction.execute(
                "INSERT INTO concept_mentions (
                    id, concept_id, card_id, selected_text, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    input.mention_id,
                    persisted_target.concept_id,
                    input.card_id,
                    persisted_target.selected_text,
                    updated_at,
                ],
            )?;
            synchronize_mentions(&transaction, &input.card_id, &persisted.concept_mentions)?;
            transaction.execute(
                "UPDATE concepts SET updated_at = ?1 WHERE id = ?2",
                params![updated_at, actual_concept_id],
            )?;

            let document_json =
                serde_json::to_string(&persisted.document).map_err(|_| concept_save_error())?;
            let content_hash = hex::encode(Sha256::digest(persisted.plain_text.trim().as_bytes()));
            transaction.execute(
                "UPDATE cards
                 SET original_content = ?1, content_hash = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![
                    persisted.plain_text,
                    content_hash,
                    updated_at,
                    input.card_id
                ],
            )?;
            transaction.execute(
                "INSERT INTO card_documents (card_id, document_json, schema_version, updated_at)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(card_id) DO UPDATE SET
                    document_json = excluded.document_json,
                    schema_version = excluded.schema_version,
                    updated_at = excluded.updated_at",
                params![input.card_id, document_json, updated_at],
            )?;
            let concept = load_concept_summary(&transaction, &actual_concept_id)?
                .ok_or_else(concept_save_error)?;
            let mention = CardConcept {
                id: concept.id.clone(),
                name: concept.name.clone(),
                explanation: concept.explanation.clone(),
                mention_id: input.mention_id.clone(),
            };
            transaction.commit()?;

            Ok(SavedConceptAnnotation {
                concept,
                mention,
                document: persisted.document,
                original_content: persisted.plain_text,
                updated_at,
            })
        })
    }
}

fn validate_annotation_input(input: &SaveConceptAnnotationInput) -> Result<(), AppError> {
    Uuid::parse_str(&input.concept_id).map_err(|_| concept_selection_stale_error())?;
    Uuid::parse_str(&input.mention_id).map_err(|_| concept_selection_stale_error())?;
    normalize_concept_name(&input.name)?;
    if input.explanation.trim().is_empty() {
        return Err(AppError::new(
            "concept_explanation_invalid",
            "概念解释不能为空",
            "填写概念解释后重试",
        ));
    }
    let selection_length = input.selected_text.chars().count();
    if input.selected_text.trim().is_empty() || selection_length > 200 {
        return Err(concept_selection_stale_error());
    }
    Ok(())
}

fn display_concept_name(value: &str) -> Result<String, AppError> {
    normalize_concept_name(value)?;
    Ok(value.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn concept_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConceptSummary> {
    Ok(ConceptSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        explanation: row.get(2)?,
        mention_count: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn load_concept_summary(
    connection: &Connection,
    id: &str,
) -> Result<Option<ConceptSummary>, AppError> {
    connection
        .query_row(
            "SELECT concepts.id, concepts.name, concepts.explanation,
                    COUNT(concept_mentions.id), concepts.updated_at
             FROM concepts
             LEFT JOIN concept_mentions ON concept_mentions.concept_id = concepts.id
             WHERE concepts.id = ?1
             GROUP BY concepts.id",
            [id],
            concept_summary_row,
        )
        .optional()
        .map_err(AppError::from)
}

fn load_concept_by_normalized_name(
    connection: &Connection,
    normalized_name: &str,
) -> Result<Option<ConceptSummary>, AppError> {
    connection
        .query_row(
            "SELECT concepts.id, concepts.name, concepts.explanation,
                    COUNT(concept_mentions.id), concepts.updated_at
             FROM concepts
             LEFT JOIN concept_mentions ON concept_mentions.concept_id = concepts.id
             WHERE concepts.normalized_name = ?1
             GROUP BY concepts.id",
            [normalized_name],
            concept_summary_row,
        )
        .optional()
        .map_err(AppError::from)
}

fn ensure_card_exists(connection: &Connection, card_id: &str) -> Result<(), AppError> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE id = ?1)",
        [card_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(card_not_found_error());
    }
    Ok(())
}

fn load_card_annotation_state(
    connection: &Connection,
    card_id: &str,
) -> Result<(CardStatus, String, String, Option<String>), AppError> {
    connection
        .query_row(
            "SELECT cards.status, cards.original_content, cards.updated_at,
                    card_documents.document_json
             FROM cards
             LEFT JOIN card_documents ON card_documents.card_id = cards.id
             WHERE cards.id = ?1",
            [card_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(card_not_found_error)
        .and_then(|(status, content, updated_at, document)| {
            Ok((
                CardStatus::from_sql(&status).ok_or_else(database_data_invalid_error)?,
                content,
                updated_at,
                document,
            ))
        })
}

fn validate_current_document(
    submitted: &Value,
    target_mention_id: &str,
    current_plain_text: &str,
    current_document_json: Option<&str>,
    preserve_target: bool,
) -> Result<(), AppError> {
    let mut without_target = submitted.clone();
    if !preserve_target {
        remove_target_mark(&mut without_target, target_mention_id)?;
    }
    canonicalize_document(&mut without_target)?;
    let validated =
        validate_document(&without_target, false).map_err(|_| concept_selection_stale_error())?;
    if validated.plain_text != current_plain_text {
        return Err(concept_selection_stale_error());
    }
    if let Some(current_json) = current_document_json {
        let mut current = serde_json::from_str::<Value>(current_json)
            .map_err(|_| database_data_invalid_error())?;
        canonicalize_document(&mut current)?;
        if current != without_target {
            return Err(concept_selection_stale_error());
        }
    }
    Ok(())
}

fn remove_target_mark(document: &mut Value, target_mention_id: &str) -> Result<(), AppError> {
    visit_text_nodes_mut(document, &mut |node| {
        let Some(marks) = node.get_mut("marks").and_then(Value::as_array_mut) else {
            return Ok(());
        };
        marks.retain(|mark| {
            mark.get("attrs")
                .and_then(|attrs| attrs.get("mentionId"))
                .and_then(Value::as_str)
                != Some(target_mention_id)
        });
        if marks.is_empty() {
            node.as_object_mut()
                .ok_or_else(concept_selection_stale_error)?
                .remove("marks");
        }
        Ok(())
    })
}

fn remove_concept_marks(document: &mut Value, concept_id: &str) -> Result<usize, AppError> {
    fn visit(value: &mut Value, concept_id: &str) -> Result<usize, AppError> {
        let Some(object) = value.as_object_mut() else {
            return Ok(0);
        };
        let mut removed = 0usize;
        let remove_marks_key = if object.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(marks) = object.get_mut("marks").and_then(Value::as_array_mut) {
                let previous_len = marks.len();
                marks.retain(|mark| {
                    mark.get("attrs")
                        .and_then(|attrs| attrs.get("conceptId"))
                        .and_then(Value::as_str)
                        != Some(concept_id)
                });
                removed += previous_len - marks.len();
                marks.is_empty()
            } else {
                false
            }
        } else {
            false
        };
        if remove_marks_key {
            object.remove("marks");
        }
        if let Some(content) = object.get_mut("content").and_then(Value::as_array_mut) {
            for child in content {
                removed += visit(child, concept_id)?;
            }
        }
        Ok(removed)
    }

    visit(document, concept_id)
}

fn replace_target_concept_id(
    document: &mut Value,
    mention_id: &str,
    expected_concept_id: &str,
    actual_concept_id: &str,
) -> Result<(), AppError> {
    let mut replacements = 0usize;
    visit_text_nodes_mut(document, &mut |node| {
        let Some(marks) = node.get_mut("marks").and_then(Value::as_array_mut) else {
            return Ok(());
        };
        for mark in marks {
            let Some(attrs) = mark.get_mut("attrs").and_then(Value::as_object_mut) else {
                continue;
            };
            if attrs.get("mentionId").and_then(Value::as_str) == Some(mention_id) {
                if attrs.get("conceptId").and_then(Value::as_str) != Some(expected_concept_id) {
                    return Err(concept_selection_stale_error());
                }
                attrs.insert(
                    "conceptId".to_owned(),
                    Value::String(actual_concept_id.to_owned()),
                );
                replacements += 1;
            }
        }
        Ok(())
    })?;
    if replacements == 0 {
        return Err(concept_selection_stale_error());
    }
    Ok(())
}

fn visit_text_nodes_mut(
    document: &mut Value,
    visitor: &mut impl FnMut(&mut Value) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let content = document
        .get_mut("content")
        .and_then(Value::as_array_mut)
        .ok_or_else(concept_selection_stale_error)?;
    for block in content {
        let Some(children) = block.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        for child in children {
            if child.get("type").and_then(Value::as_str) == Some("text") {
                visitor(child)?;
            }
        }
    }
    Ok(())
}

fn canonicalize_document(document: &mut Value) -> Result<(), AppError> {
    fn canonicalize_content(node: &mut Value) -> Result<(), AppError> {
        let Some(children) = node.get_mut("content").and_then(Value::as_array_mut) else {
            return Ok(());
        };
        for child in children.iter_mut() {
            canonicalize_content(child)?;
        }
        let mut merged: Vec<Value> = Vec::with_capacity(children.len());
        for child in std::mem::take(children) {
            let can_merge = child.get("type").and_then(Value::as_str) == Some("text")
                && merged.last().is_some_and(|previous| {
                    previous.get("type").and_then(Value::as_str) == Some("text")
                        && previous.get("marks") == child.get("marks")
                });
            if can_merge {
                let text = child
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(concept_selection_stale_error)?;
                let previous = merged
                    .last_mut()
                    .ok_or_else(concept_selection_stale_error)?;
                let previous_text = previous
                    .get_mut("text")
                    .and_then(|value| value.as_str())
                    .ok_or_else(concept_selection_stale_error)?
                    .to_owned();
                previous
                    .as_object_mut()
                    .ok_or_else(concept_selection_stale_error)?
                    .insert("text".to_owned(), Value::String(previous_text + text));
            } else {
                merged.push(child);
            }
        }
        *children = merged;
        Ok(())
    }

    if document.get("content").and_then(Value::as_array).is_none() {
        return Err(concept_selection_stale_error());
    }
    canonicalize_content(document)
}

fn validate_existing_images(
    connection: &Connection,
    card_id: &str,
    submitted_ids: &[String],
) -> Result<(), AppError> {
    let mut statement = connection.prepare(
        "SELECT attachment_id FROM card_document_images
         WHERE card_id = ?1 ORDER BY position ASC",
    )?;
    let current = statement
        .query_map([card_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if current != submitted_ids {
        return Err(concept_selection_stale_error());
    }
    Ok(())
}

fn validate_existing_mentions(
    connection: &Connection,
    card_id: &str,
    mentions: &[DocumentMention],
    target_mention_id: &str,
) -> Result<(), AppError> {
    for mention in mentions
        .iter()
        .filter(|mention| mention.mention_id != target_mention_id)
    {
        let valid = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM concept_mentions
                WHERE id = ?1 AND card_id = ?2 AND concept_id = ?3
                  AND selected_text = ?4
             )",
            params![
                mention.mention_id,
                card_id,
                mention.concept_id,
                mention.selected_text
            ],
            |row| row.get::<_, bool>(0),
        )?;
        if !valid {
            return Err(concept_selection_stale_error());
        }
    }
    Ok(())
}

fn synchronize_mentions(
    connection: &Connection,
    card_id: &str,
    mentions: &[DocumentMention],
) -> Result<(), AppError> {
    for mention in mentions {
        let changed = connection.execute(
            "UPDATE concept_mentions
             SET selected_text = ?1
             WHERE id = ?2 AND card_id = ?3 AND concept_id = ?4",
            params![
                mention.selected_text,
                mention.mention_id,
                card_id,
                mention.concept_id
            ],
        )?;
        if changed != 1 {
            return Err(concept_selection_stale_error());
        }
    }
    let retained_ids = mentions
        .iter()
        .map(|mention| mention.mention_id.as_str())
        .collect::<Vec<_>>();
    if retained_ids.is_empty() {
        connection.execute("DELETE FROM concept_mentions WHERE card_id = ?1", [card_id])?;
    } else {
        let placeholders = std::iter::repeat_n("?", retained_ids.len())
            .enumerate()
            .map(|(index, _)| format!("?{}", index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "DELETE FROM concept_mentions
             WHERE card_id = ?1 AND id NOT IN ({placeholders})"
        );
        let mut values = Vec::with_capacity(retained_ids.len() + 1);
        values.push(card_id);
        values.extend(retained_ids);
        connection.execute(&sql, rusqlite::params_from_iter(values))?;
    }
    Ok(())
}

fn monotonic_utc_timestamp(previous: &str, candidate: DateTime<Utc>) -> Result<String, AppError> {
    let previous = DateTime::parse_from_rfc3339(previous)
        .map_err(|_| database_data_invalid_error())?
        .with_timezone(&Utc);
    let timestamp = if candidate > previous {
        candidate
    } else {
        previous
            .checked_add_signed(TimeDelta::nanoseconds(1))
            .ok_or_else(database_data_invalid_error)?
    };
    Ok(timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true))
}

fn card_not_found_error() -> AppError {
    AppError::new(
        "card_not_found",
        "找不到这张卡片",
        "返回收集箱后刷新卡片列表",
    )
}

fn card_archived_read_only_error() -> AppError {
    AppError::new(
        "card_archived_read_only",
        "已归档卡片不能新增概念注释",
        "先恢复卡片后再试",
    )
}

fn concept_not_found_error() -> AppError {
    AppError::new(
        "concept_not_found",
        "找不到这个概念",
        "返回概念集合后刷新列表",
    )
}

fn concept_selection_stale_error() -> AppError {
    AppError::new(
        "concept_selection_stale",
        "原文选区已发生变化",
        "重新选择文字后再解释",
    )
}

fn concept_save_error() -> AppError {
    AppError::new(
        "concept_save_failed",
        "无法保存概念注释",
        "保留当前解释并重试",
    )
}

fn concept_delete_error() -> AppError {
    AppError::new(
        "concept_delete_failed",
        "无法删除概念",
        "概念和卡片内容均未更改，请重试",
    )
}

fn database_data_invalid_error() -> AppError {
    AppError::new(
        "database_data_invalid",
        "数据库中的概念数据无效",
        "重新打开应用；问题持续时检查数据目录",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use tempfile::tempdir;

    use super::ConceptRepository;
    use crate::{
        database::Database,
        domain::{
            card::{CaptureMethod, CardStatus, NewCardInput, SourceType},
            concept::SaveConceptAnnotationInput,
        },
        repository::{
            attachment_repository::AttachmentRepository, card_repository::CardRepository,
            document_repository::DocumentRepository,
        },
    };

    const FIRST_CONCEPT_ID: &str = "8ee5e2b0-6f71-4c31-9d47-34c6974f8c01";
    const SECOND_CONCEPT_ID: &str = "5ab6fb61-f982-4a0a-9093-eef367b332e0";
    const THIRD_CONCEPT_ID: &str = "6f9d2d76-c9fd-45a2-8e47-dba791671b6c";
    const FIRST_MENTION_ID: &str = "7fd7d15d-6672-471f-afc8-a8b4cdf1ed0f";
    const SECOND_MENTION_ID: &str = "a2731af3-22ba-459a-9286-2543ac8e9338";
    const THIRD_MENTION_ID: &str = "bb778418-af3b-4f61-973a-c8677e4e712a";

    struct Fixture {
        _directory: tempfile::TempDir,
        database: Database,
        attachments: AttachmentRepository,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempdir().unwrap();
            let database = Database::open_in_memory().unwrap();
            let attachments = AttachmentRepository::new(
                directory.path().join("attachments"),
                directory.path().join("cache"),
            );
            Self {
                _directory: directory,
                database,
                attachments,
            }
        }

        fn create_card(&self, title: &str, content: &str) -> String {
            let card = CardRepository::new(&self.database)
                .create(NewCardInput {
                    title: title.to_owned(),
                    original_content: content.to_owned(),
                    note: String::new(),
                    source_type: SourceType::Personal,
                    capture_method: CaptureMethod::Quick,
                    source_name: String::new(),
                    source_uri: String::new(),
                })
                .unwrap()
                .card;
            DocumentRepository::new(&self.database, &self.attachments)
                .save_card_document(&card.id, plain_document(content))
                .unwrap();
            card.id
        }

        fn repository(&self) -> ConceptRepository<'_> {
            ConceptRepository::new(&self.database)
        }
    }

    fn plain_document(text: &str) -> Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    fn marked_document(concept_id: &str, mention_id: &str, text: &str) -> Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": text,
                    "marks": [{
                        "type": "concept",
                        "attrs": {
                            "conceptId": concept_id,
                            "mentionId": mention_id
                        }
                    }]
                }]
            }]
        })
    }

    fn dual_concept_document(include_second_mark: bool) -> Value {
        let second_marks = include_second_mark.then(|| {
            json!([{
                "type": "concept",
                "attrs": {
                    "conceptId": SECOND_CONCEPT_ID,
                    "mentionId": SECOND_MENTION_ID
                }
            }])
        });
        let mut second_node = json!({
            "type": "text",
            "text": "概念二"
        });
        if let Some(marks) = second_marks {
            second_node
                .as_object_mut()
                .unwrap()
                .insert("marks".to_owned(), marks);
        }
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    {
                        "type": "text",
                        "text": "概念一",
                        "marks": [{
                            "type": "concept",
                            "attrs": {
                                "conceptId": FIRST_CONCEPT_ID,
                                "mentionId": FIRST_MENTION_ID
                            }
                        }]
                    },
                    {"type": "text", "text": "与"},
                    second_node
                ]
            }]
        })
    }

    fn input(
        card_id: &str,
        concept_id: &str,
        mention_id: &str,
        selected_text: &str,
        name: &str,
        explanation: &str,
        document: Value,
    ) -> SaveConceptAnnotationInput {
        SaveConceptAnnotationInput {
            card_id: card_id.to_owned(),
            concept_id: concept_id.to_owned(),
            mention_id: mention_id.to_owned(),
            selected_text: selected_text.to_owned(),
            name: name.to_owned(),
            explanation: explanation.to_owned(),
            overwrite_existing: false,
            document,
        }
    }

    #[test]
    fn saves_a_new_concept_mention_and_marked_document_atomically() {
        let fixture = Fixture::new();
        let card_id = fixture.create_card("学习笔记", "费曼学习法");
        let saved = fixture
            .repository()
            .save_concept_annotation(input(
                &card_id,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "费曼学习法",
                "费曼学习法",
                "通过讲解检验自己是否真正理解。",
                marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "费曼学习法"),
            ))
            .unwrap();

        assert_eq!(saved.concept.id, FIRST_CONCEPT_ID);
        assert_eq!(saved.concept.mention_count, 1);
        assert_eq!(saved.mention.mention_id, FIRST_MENTION_ID);
        assert_eq!(
            saved.document["content"][0]["content"][0]["marks"][0]["attrs"]["conceptId"],
            FIRST_CONCEPT_ID
        );
        fixture
            .database
            .with_connection(|connection| {
                let rows = (
                    connection.query_row(
                        "SELECT COUNT(*) FROM concepts WHERE id = ?1",
                        [FIRST_CONCEPT_ID],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM concept_mentions
                         WHERE id = ?1 AND concept_id = ?2 AND card_id = ?3
                           AND selected_text = '费曼学习法'",
                        rusqlite::params![FIRST_MENTION_ID, FIRST_CONCEPT_ID, card_id],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM card_documents
                         WHERE card_id = ?1 AND document_json LIKE '%mentionId%'",
                        [&card_id],
                        |row| row.get::<_, i64>(0),
                    )?,
                );
                assert_eq!(rows, (1, 1, 1));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn explicitly_overwrites_an_existing_explanation_without_adding_a_mention() {
        let fixture = Fixture::new();
        let card_id = fixture.create_card("创业笔记", "OPC");
        let document = marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "OPC");
        fixture
            .repository()
            .save_concept_annotation(input(
                &card_id,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "OPC",
                "OPC",
                "原解释。",
                document.clone(),
            ))
            .unwrap();
        let mut overwrite = input(
            &card_id,
            FIRST_CONCEPT_ID,
            FIRST_MENTION_ID,
            "OPC",
            "OPC",
            "新的解释。",
            document.clone(),
        );
        overwrite.overwrite_existing = true;

        let saved = fixture
            .repository()
            .save_concept_annotation(overwrite)
            .unwrap();

        assert_eq!(saved.concept.explanation, "新的解释。");
        assert_eq!(saved.concept.mention_count, 1);
        assert_eq!(saved.mention.mention_id, FIRST_MENTION_ID);
        assert_eq!(saved.document, document);
        let card_concepts = fixture.repository().list_card_concepts(&card_id).unwrap();
        assert_eq!(card_concepts.len(), 1);
        assert_eq!(card_concepts[0].explanation, "新的解释。");
    }

    #[test]
    fn database_failure_rolls_back_concept_mention_and_document_without_leaking_details() {
        let fixture = Fixture::new();
        let card_id = fixture.create_card("事务测试", "费曼学习法");
        fixture
            .database
            .with_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER fail_concept_document_save
                     BEFORE UPDATE ON card_documents
                     WHEN NEW.document_json LIKE '%mentionId%'
                     BEGIN
                       SELECT RAISE(ABORT, 'SQL /private/IdeaCard SENTINEL');
                     END;",
                )?;
                Ok(())
            })
            .unwrap();

        let error = fixture
            .repository()
            .save_concept_annotation(input(
                &card_id,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "费曼学习法",
                "费曼学习法",
                "通过讲解检验自己是否真正理解。",
                marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "费曼学习法"),
            ))
            .unwrap_err();

        assert_eq!(error.code, "database_operation_failed");
        assert!(!format!("{error:?}").contains("SENTINEL"));
        fixture
            .database
            .with_connection(|connection| {
                let counts = (
                    connection.query_row("SELECT COUNT(*) FROM concepts", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    connection.query_row("SELECT COUNT(*) FROM concept_mentions", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                );
                let document = connection.query_row(
                    "SELECT document_json FROM card_documents WHERE card_id = ?1",
                    [&card_id],
                    |row| row.get::<_, String>(0),
                )?;
                assert_eq!(counts, (0, 0));
                assert!(!document.contains("mentionId"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn reuses_a_normalized_name_without_overwriting_the_existing_explanation() {
        let fixture = Fixture::new();
        let first_card = fixture.create_card("第一张", "费曼学习法");
        fixture
            .repository()
            .save_concept_annotation(input(
                &first_card,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "费曼学习法",
                "费曼 学习法",
                "原有解释",
                marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "费曼学习法"),
            ))
            .unwrap();
        let second_card = fixture.create_card("第二张", "另一处引用");

        let saved = fixture
            .repository()
            .save_concept_annotation(input(
                &second_card,
                SECOND_CONCEPT_ID,
                SECOND_MENTION_ID,
                "另一处引用",
                "  费曼\u{3000}学习法 ",
                "不应覆盖",
                marked_document(SECOND_CONCEPT_ID, SECOND_MENTION_ID, "另一处引用"),
            ))
            .unwrap();

        assert_eq!(saved.concept.id, FIRST_CONCEPT_ID);
        assert_eq!(saved.concept.explanation, "原有解释");
        assert_eq!(saved.concept.mention_count, 2);
        assert_eq!(
            saved.document["content"][0]["content"][0]["marks"][0]["attrs"]["conceptId"],
            FIRST_CONCEPT_ID
        );
        fixture
            .database
            .with_connection(|connection| {
                let concepts =
                    connection.query_row("SELECT COUNT(*) FROM concepts", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let stored_document = connection.query_row(
                    "SELECT document_json FROM card_documents WHERE card_id = ?1",
                    [&second_card],
                    |row| row.get::<_, String>(0),
                )?;
                assert_eq!(concepts, 1);
                assert!(stored_document.contains(FIRST_CONCEPT_ID));
                assert!(!stored_document.contains(SECOND_CONCEPT_ID));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn rejects_a_missing_or_mismatched_target_mark_and_rolls_back_all_rows() {
        let fixture = Fixture::new();
        let card_id = fixture.create_card("过期选区", "原始概念");
        for document in [
            plain_document("原始概念"),
            marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "另一段文字"),
            marked_document(SECOND_CONCEPT_ID, FIRST_MENTION_ID, "原始概念"),
            marked_document(FIRST_CONCEPT_ID, SECOND_MENTION_ID, "原始概念"),
        ] {
            let error = fixture
                .repository()
                .save_concept_annotation(input(
                    &card_id,
                    FIRST_CONCEPT_ID,
                    FIRST_MENTION_ID,
                    "原始概念",
                    "原始概念",
                    "解释",
                    document,
                ))
                .unwrap_err();
            assert_eq!(error.code, "concept_selection_stale");
        }

        fixture
            .database
            .with_connection(|connection| {
                let counts = (
                    connection.query_row("SELECT COUNT(*) FROM concepts", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    connection.query_row("SELECT COUNT(*) FROM concept_mentions", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                );
                assert_eq!(counts, (0, 0));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn lists_counts_card_concepts_and_both_inbox_and_archived_sources() {
        let fixture = Fixture::new();
        let inbox_card = fixture.create_card("收集箱卡片", "概念一");
        let archived_card = fixture.create_card("归档卡片", "概念二");
        for (card_id, mention_id, text) in [
            (&inbox_card, FIRST_MENTION_ID, "概念一"),
            (&archived_card, SECOND_MENTION_ID, "概念二"),
        ] {
            fixture
                .repository()
                .save_concept_annotation(input(
                    card_id,
                    FIRST_CONCEPT_ID,
                    mention_id,
                    text,
                    "长期 概念",
                    "稳定解释",
                    marked_document(FIRST_CONCEPT_ID, mention_id, text),
                ))
                .unwrap();
        }
        fixture
            .database
            .with_connection(|connection| {
                connection.execute(
                    "UPDATE cards SET status = 'archived', archived_at = updated_at
                     WHERE id = ?1",
                    [&archived_card],
                )?;
                Ok(())
            })
            .unwrap();

        let summaries = fixture.repository().list_concepts().unwrap();
        let detail = fixture.repository().get_concept(FIRST_CONCEPT_ID).unwrap();
        let card_concepts = fixture
            .repository()
            .list_card_concepts(&archived_card)
            .unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].mention_count, 2);
        assert_eq!(detail.mentions.len(), 2);
        assert!(detail
            .mentions
            .iter()
            .any(|source| source.card_status == CardStatus::Archived));
        assert!(detail
            .mentions
            .iter()
            .any(|source| source.card_status != CardStatus::Archived));
        assert_eq!(card_concepts.len(), 1);
        assert_eq!(card_concepts[0].mention_id, SECOND_MENTION_ID);
    }

    #[test]
    fn deletes_a_concept_and_all_marks_without_changing_card_text_or_other_concepts() {
        let fixture = Fixture::new();
        let first_card = fixture.create_card("双概念卡片", "概念一与概念二");
        fixture
            .repository()
            .save_concept_annotation(input(
                &first_card,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "概念一",
                "概念一",
                "第一个解释",
                dual_concept_document(false),
            ))
            .unwrap();
        fixture
            .repository()
            .save_concept_annotation(input(
                &first_card,
                SECOND_CONCEPT_ID,
                SECOND_MENTION_ID,
                "概念二",
                "概念二",
                "第二个解释",
                dual_concept_document(true),
            ))
            .unwrap();

        let second_card = fixture.create_card("另一张卡片", "再次引用概念一");
        fixture
            .repository()
            .save_concept_annotation(input(
                &second_card,
                THIRD_CONCEPT_ID,
                THIRD_MENTION_ID,
                "再次引用概念一",
                "概念一",
                "不会覆盖原解释",
                marked_document(THIRD_CONCEPT_ID, THIRD_MENTION_ID, "再次引用概念一"),
            ))
            .unwrap();

        fixture
            .repository()
            .delete_concept(FIRST_CONCEPT_ID)
            .unwrap();

        let concepts = fixture.repository().list_concepts().unwrap();
        let first_card_concepts = fixture
            .repository()
            .list_card_concepts(&first_card)
            .unwrap();
        let second_card_concepts = fixture
            .repository()
            .list_card_concepts(&second_card)
            .unwrap();
        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].id, SECOND_CONCEPT_ID);
        assert_eq!(first_card_concepts.len(), 1);
        assert_eq!(first_card_concepts[0].id, SECOND_CONCEPT_ID);
        assert!(second_card_concepts.is_empty());
        assert_eq!(
            fixture
                .repository()
                .get_concept(FIRST_CONCEPT_ID)
                .unwrap_err()
                .code,
            "concept_not_found"
        );

        fixture
            .database
            .with_connection(|connection| {
                let first = connection.query_row(
                    "SELECT cards.original_content, card_documents.document_json
                     FROM cards
                     JOIN card_documents ON card_documents.card_id = cards.id
                     WHERE cards.id = ?1",
                    [&first_card],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?;
                let second = connection.query_row(
                    "SELECT cards.original_content, card_documents.document_json
                     FROM cards
                     JOIN card_documents ON card_documents.card_id = cards.id
                     WHERE cards.id = ?1",
                    [&second_card],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?;
                assert_eq!(first.0, "概念一与概念二");
                assert_eq!(second.0, "再次引用概念一");
                assert!(!first.1.contains(FIRST_CONCEPT_ID));
                assert!(!first.1.contains(FIRST_MENTION_ID));
                assert!(first.1.contains(SECOND_CONCEPT_ID));
                assert!(first.1.contains(SECOND_MENTION_ID));
                assert!(!second.1.contains(FIRST_CONCEPT_ID));
                assert!(!second.1.contains(THIRD_MENTION_ID));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn delete_failure_rolls_back_the_concept_mentions_and_document_marks() {
        let fixture = Fixture::new();
        let card_id = fixture.create_card("回滚测试", "需要保留的概念");
        fixture
            .repository()
            .save_concept_annotation(input(
                &card_id,
                FIRST_CONCEPT_ID,
                FIRST_MENTION_ID,
                "需要保留的概念",
                "保留概念",
                "稳定解释",
                marked_document(FIRST_CONCEPT_ID, FIRST_MENTION_ID, "需要保留的概念"),
            ))
            .unwrap();
        fixture
            .database
            .with_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER fail_concept_delete
                     BEFORE DELETE ON concepts
                     BEGIN
                       SELECT RAISE(ABORT, 'SQL /private/IdeaCard DELETE SENTINEL');
                     END;",
                )?;
                Ok(())
            })
            .unwrap();

        let error = fixture
            .repository()
            .delete_concept(FIRST_CONCEPT_ID)
            .unwrap_err();
        assert_eq!(error.code, "database_operation_failed");
        assert!(!format!("{error:?}").contains("SENTINEL"));

        fixture
            .database
            .with_connection(|connection| {
                let counts = (
                    connection.query_row(
                        "SELECT COUNT(*) FROM concepts WHERE id = ?1",
                        [FIRST_CONCEPT_ID],
                        |row| row.get::<_, i64>(0),
                    )?,
                    connection.query_row(
                        "SELECT COUNT(*) FROM concept_mentions WHERE concept_id = ?1",
                        [FIRST_CONCEPT_ID],
                        |row| row.get::<_, i64>(0),
                    )?,
                );
                let document = connection.query_row(
                    "SELECT document_json FROM card_documents WHERE card_id = ?1",
                    [&card_id],
                    |row| row.get::<_, String>(0),
                )?;
                assert_eq!(counts, (1, 1));
                assert!(document.contains(FIRST_CONCEPT_ID));
                assert!(document.contains(FIRST_MENTION_ID));
                Ok(())
            })
            .unwrap();
    }
}
