use serde::Deserialize;
use tauri::AppHandle;

use crate::{app_error::AppError, native};

const MAX_FIELD_ID_LENGTH: usize = 128;
const MAX_FIELD_VALUE_LENGTH: usize = 20_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeTextFieldInput {
    field_id: String,
    value: String,
    #[serde(default)]
    placeholder: String,
    #[serde(default)]
    read_only: bool,
    visible: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
pub(crate) fn sync_native_text_field(
    input: NativeTextFieldInput,
    app: AppHandle,
) -> Result<(), AppError> {
    validate(&input)?;
    native::native_text_field::sync(
        &app,
        native::native_text_field::NativeTextFieldConfig {
            field_id: input.field_id,
            value: input.value,
            placeholder: input.placeholder,
            read_only: input.read_only,
            visible: input.visible,
            x: input.x,
            y: input.y,
            width: input.width,
            height: input.height,
        },
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_native_text_field(field_id: String, app: AppHandle) -> Result<(), AppError> {
    validate_field_id(&field_id)?;
    native::native_text_field::remove(&app, field_id);
    Ok(())
}

#[tauri::command]
pub(crate) fn focus_native_text_field(field_id: String, app: AppHandle) -> Result<(), AppError> {
    validate_field_id(&field_id)?;
    native::native_text_field::focus(&app, field_id);
    Ok(())
}

fn validate(input: &NativeTextFieldInput) -> Result<(), AppError> {
    validate_field_id(&input.field_id)?;
    if input.value.chars().count() > MAX_FIELD_VALUE_LENGTH
        || input.placeholder.chars().count() > MAX_FIELD_VALUE_LENGTH
        || !input.x.is_finite()
        || !input.y.is_finite()
        || !input.width.is_finite()
        || !input.height.is_finite()
        || input.width < 0.0
        || input.height < 0.0
    {
        return Err(native_text_field_error());
    }
    Ok(())
}

fn validate_field_id(field_id: &str) -> Result<(), AppError> {
    let field_id = field_id.trim();
    if field_id.is_empty() || field_id.chars().count() > MAX_FIELD_ID_LENGTH {
        return Err(native_text_field_error());
    }
    Ok(())
}

fn native_text_field_error() -> AppError {
    AppError::new(
        "native_text_field_invalid",
        "原生输入框暂时不可用",
        "重新打开卡片后再试",
    )
}

#[cfg(test)]
mod tests {
    use super::{validate, NativeTextFieldInput};

    fn valid_input() -> NativeTextFieldInput {
        NativeTextFieldInput {
            field_id: "card-title-1".to_owned(),
            value: "标题".to_owned(),
            placeholder: String::new(),
            read_only: false,
            visible: true,
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 35.0,
        }
    }

    #[test]
    fn accepts_a_finite_native_text_field() {
        assert!(validate(&valid_input()).is_ok());
    }

    #[test]
    fn rejects_invalid_ids_and_geometry() {
        let mut input = valid_input();
        input.field_id = " ".to_owned();
        assert!(validate(&input).is_err());

        let mut input = valid_input();
        input.width = f64::NAN;
        assert!(validate(&input).is_err());
    }
}
