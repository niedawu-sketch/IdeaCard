use crate::app_error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::card::CardStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptSummary {
    pub id: String,
    pub name: String,
    pub explanation: String,
    pub mention_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptSource {
    pub mention_id: String,
    pub card_id: String,
    pub card_title: String,
    pub selected_text: String,
    pub card_status: CardStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptDetail {
    pub id: String,
    pub name: String,
    pub explanation: String,
    pub mention_count: i64,
    pub updated_at: String,
    pub mentions: Vec<ConceptSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardConcept {
    pub id: String,
    pub name: String,
    pub explanation: String,
    pub mention_id: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveConceptAnnotationInput {
    pub card_id: String,
    pub concept_id: String,
    pub mention_id: String,
    pub selected_text: String,
    pub name: String,
    pub explanation: String,
    pub overwrite_existing: bool,
    pub document: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedConceptAnnotation {
    pub concept: ConceptSummary,
    pub mention: CardConcept,
    pub document: Value,
    pub original_content: String,
    pub updated_at: String,
}

pub fn normalize_concept_name(value: &str) -> Result<String, AppError> {
    let normalized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() {
        return Err(AppError::new(
            "concept_name_invalid",
            "概念名称不能为空",
            "填写概念名称后重试",
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{normalize_concept_name, ConceptSummary, SaveConceptAnnotationInput};

    #[test]
    fn normalizes_unicode_case_and_internal_whitespace() {
        assert_eq!(
            normalize_concept_name("  Straße\u{3000}  学习法  ").unwrap(),
            "straße 学习法"
        );
    }

    #[test]
    fn rejects_an_empty_concept_name_without_echoing_input() {
        let error = normalize_concept_name(" \n\t ").unwrap_err();

        assert_eq!(error.code, "concept_name_invalid");
        assert_eq!(error.message, "概念名称不能为空");
    }

    #[test]
    fn command_dtos_use_camel_case_without_leaking_internal_fields() {
        let input = SaveConceptAnnotationInput {
            card_id: "card-1".to_owned(),
            concept_id: "concept-1".to_owned(),
            mention_id: "mention-1".to_owned(),
            selected_text: "费曼学习法".to_owned(),
            name: "费曼学习法".to_owned(),
            explanation: "通过讲解检验理解。".to_owned(),
            overwrite_existing: false,
            document: json!({"type": "doc", "content": []}),
        };
        let summary = ConceptSummary {
            id: "concept-1".to_owned(),
            name: "费曼学习法".to_owned(),
            explanation: "通过讲解检验理解。".to_owned(),
            mention_count: 2,
            updated_at: "2026-07-19T08:00:00Z".to_owned(),
        };

        assert_eq!(
            serde_json::to_value(input).unwrap(),
            json!({
                "cardId": "card-1",
                "conceptId": "concept-1",
                "mentionId": "mention-1",
                "selectedText": "费曼学习法",
                "name": "费曼学习法",
                "explanation": "通过讲解检验理解。",
                "overwriteExisting": false,
                "document": {"type": "doc", "content": []}
            })
        );
        assert_eq!(
            serde_json::to_value(summary).unwrap()["mentionCount"],
            json!(2)
        );
    }
}
