use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Runtime,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const QUICK_SHORTCUT: &str = "Control+Alt+Space";
const CLIPBOARD_SHORTCUT: &str = "Control+Alt+V";
const SCREENSHOT_SHORTCUT: &str = "Control+1";

pub(crate) const SHORTCUTS: [(&str, CaptureAction); 3] = [
    (QUICK_SHORTCUT, CaptureAction::Quick),
    (CLIPBOARD_SHORTCUT, CaptureAction::Clipboard),
    (SCREENSHOT_SHORTCUT, CaptureAction::Screenshot),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureAction {
    Quick,
    Clipboard,
    Screenshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutStatus {
    shortcut: &'static str,
    action: CaptureAction,
    registered: bool,
}

pub fn action_for_shortcut(shortcut: &str) -> Option<CaptureAction> {
    match shortcut {
        QUICK_SHORTCUT => Some(CaptureAction::Quick),
        CLIPBOARD_SHORTCUT => Some(CaptureAction::Clipboard),
        SCREENSHOT_SHORTCUT => Some(CaptureAction::Screenshot),
        _ => None,
    }
}

pub fn setup_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let quick = MenuItem::with_id(app, "capture_quick", "快速记录", true, Some(QUICK_SHORTCUT))?;
    let clipboard = MenuItem::with_id(
        app,
        "capture_clipboard",
        "捕获剪贴板",
        true,
        Some(CLIPBOARD_SHORTCUT),
    )?;
    let screenshot = MenuItem::with_id(
        app,
        "capture_screenshot",
        "截图并识别",
        true,
        Some(SCREENSHOT_SHORTCUT),
    )?;
    let open = MenuItem::with_id(app, "open_main", "打开 IdeaCard", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&quick, &clipboard, &screenshot, &open, &separator, &quit],
    )?;

    TrayIconBuilder::new()
        .title("IdeaCard")
        .tooltip("IdeaCard")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "capture_quick" => emit_capture(app, CaptureAction::Quick),
            "capture_clipboard" => emit_capture(app, CaptureAction::Clipboard),
            "capture_screenshot" => emit_capture(app, CaptureAction::Screenshot),
            "open_main" => restore_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn register_shortcuts<R: Runtime>(app: &AppHandle<R>) {
    for (shortcut, action) in SHORTCUTS {
        let result = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    emit_capture(app, action);
                }
            });
        let _ = app.emit(
            "shortcut://status",
            ShortcutStatus {
                shortcut,
                action,
                registered: result.is_ok(),
            },
        );
    }
}

fn emit_capture<R: Runtime>(app: &AppHandle<R>, action: CaptureAction) {
    if action != CaptureAction::Screenshot {
        restore_main_window(app);
    }
    let event = match action {
        CaptureAction::Quick => "capture://quick",
        CaptureAction::Clipboard => "capture://clipboard",
        CaptureAction::Screenshot => "capture://screenshot",
    };
    let _ = app.emit(event, ());
}

pub(crate) fn restore_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::{action_for_shortcut, CaptureAction};

    #[test]
    fn maps_only_the_three_fixed_shortcuts() {
        assert_eq!(
            action_for_shortcut("Control+Alt+Space"),
            Some(CaptureAction::Quick)
        );
        assert_eq!(
            action_for_shortcut("Control+Alt+V"),
            Some(CaptureAction::Clipboard)
        );
        assert_eq!(
            action_for_shortcut("Control+1"),
            Some(CaptureAction::Screenshot)
        );
        assert_eq!(action_for_shortcut("Control+Q"), None);
        assert_eq!(action_for_shortcut("Control+Alt+X"), None);
    }
}
