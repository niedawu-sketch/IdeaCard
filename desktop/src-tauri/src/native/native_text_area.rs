#[derive(Clone, Debug)]
pub struct NativeTextAreaConfig {
    pub field_id: String,
    pub value: String,
    pub read_only: bool,
    pub submit_on_enter: bool,
    pub visible: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(target_os = "macos")]
mod imp {
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        ptr::NonNull,
    };

    use block2::RcBlock;
    use objc2::{
        define_class, msg_send,
        rc::Retained,
        runtime::{ProtocolObject, Sel},
        sel, AnyThread, DeclaredClass, MainThreadOnly,
    };
    use objc2_app_kit::{
        NSApplication, NSAutoresizingMaskOptions, NSBezierPath, NSBorderType, NSColor,
        NSControlSize, NSEvent, NSEventModifierFlags, NSFont, NSScrollView, NSScroller,
        NSScrollerPart, NSScrollerStyle, NSTextDelegate, NSTextDidBeginEditingNotification,
        NSTextDidChangeNotification, NSTextDidEndEditingNotification, NSTextInputClient,
        NSTextView, NSTextViewDelegate, NSTrackingArea, NSTrackingAreaOptions, NSWindow,
        NSWindowOrderingMode,
    };
    use objc2_foundation::{
        MainThreadMarker, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol,
        NSPoint, NSRect, NSSize, NSString,
    };
    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager};

    use super::NativeTextAreaConfig;

    const SCROLLER_KNOB_WIDTH: f64 = 6.0;

    #[derive(Debug, Default)]
    struct SubtleScrollerIvars {
        hovered: Cell<bool>,
    }

    define_class!(
        #[unsafe(super(NSScroller))]
        #[name = "IdeaCardSubtleScroller"]
        #[thread_kind = MainThreadOnly]
        #[ivars = SubtleScrollerIvars]
        struct SubtleScroller;

        impl SubtleScroller {
            #[unsafe(method(setAlphaValue:))]
            fn keep_visible(&self, _alpha: f64) {
                unsafe {
                    let _: () = msg_send![super(self), setAlphaValue: 1.0_f64];
                }
            }

            #[unsafe(method(drawKnob))]
            fn draw_knob(&self) {
                let knob_rect = self.rectForPart(NSScrollerPart::Knob);
                if knob_rect.size.width <= 0.0 || knob_rect.size.height <= 0.0 {
                    return;
                }

                let width = SCROLLER_KNOB_WIDTH.min(knob_rect.size.width);
                let vertical_inset = 1.0_f64.min(knob_rect.size.height / 2.0);
                let visible_rect = NSRect::new(
                    NSPoint::new(
                        knob_rect.origin.x + (knob_rect.size.width - width) / 2.0,
                        knob_rect.origin.y + vertical_inset,
                    ),
                    NSSize::new(width, (knob_rect.size.height - vertical_inset * 2.0).max(1.0)),
                );
                let (red, green, blue, alpha) =
                    scroller_knob_color(self.ivars().hovered.get());
                let color =
                    NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, alpha);
                color.setFill();
                let radius = width / 2.0;
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    visible_rect,
                    radius,
                    radius,
                )
                .fill();
            }

            #[unsafe(method(drawKnobSlotInRect:highlight:))]
            fn draw_knob_slot(&self, _slot_rect: NSRect, _highlight: bool) {}

            #[unsafe(method(mouseEntered:))]
            fn mouse_entered(&self, _event: &NSEvent) {
                self.ivars().hovered.set(true);
                self.request_redraw();
            }

            #[unsafe(method(mouseExited:))]
            fn mouse_exited(&self, _event: &NSEvent) {
                self.ivars().hovered.set(false);
                self.request_redraw();
            }

            #[unsafe(method(updateTrackingAreas))]
            fn update_tracking_areas(&self) {
                unsafe {
                    for area in self.trackingAreas() {
                        self.removeTrackingArea(&area);
                    }
                    let _: () = msg_send![super(self), updateTrackingAreas];
                    let options = NSTrackingAreaOptions::MouseEnteredAndExited
                        | NSTrackingAreaOptions::ActiveAlways
                        | NSTrackingAreaOptions::InVisibleRect;
                    let area = NSTrackingArea::initWithRect_options_owner_userInfo(
                        NSTrackingArea::alloc(),
                        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                        options,
                        Some(self),
                        None,
                    );
                    self.addTrackingArea(&area);
                }
            }
        }
    );

    impl SubtleScroller {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(SubtleScrollerIvars::default());
            let this: Retained<Self> = unsafe {
                msg_send![
                    super(this),
                    initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0))
                ]
            };
            this.setControlSize(NSControlSize::Small);
            this.setScrollerStyle(NSScrollerStyle::Overlay);
            this.request_redraw();
            this
        }

        fn request_redraw(&self) {
            unsafe {
                let _: () = msg_send![self, setNeedsDisplay: true];
            }
        }
    }

    fn scroller_knob_color(hovered: bool) -> (f64, f64, f64, f64) {
        if hovered {
            (0.282, 0.310, 0.286, 0.58)
        } else {
            (0.392, 0.420, 0.396, 0.36)
        }
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ChangedPayload {
        field_id: String,
        value: String,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct FocusPayload {
        field_id: String,
        focused: bool,
    }

    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SubmittedPayload {
        field_id: String,
    }

    struct TextAreaDelegateIvars {
        app: AppHandle,
        field_id: String,
        submit_on_enter: Cell<bool>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "IdeaCardTextAreaDelegate"]
        #[thread_kind = MainThreadOnly]
        #[ivars = TextAreaDelegateIvars]
        struct TextAreaDelegate;

        unsafe impl NSObjectProtocol for TextAreaDelegate {}

        unsafe impl NSTextDelegate for TextAreaDelegate {}

        unsafe impl NSTextViewDelegate for TextAreaDelegate {
            #[unsafe(method(textView:doCommandBySelector:))]
            unsafe fn handle_command(&self, text_view: &NSTextView, command_selector: Sel) -> bool {
                if command_selector != sel!(insertNewline:)
                    || !self.ivars().submit_on_enter.get()
                    || text_view.hasMarkedText()
                {
                    return false.into();
                }
                let shift_pressed = NSApplication::sharedApplication(self.mtm())
                    .currentEvent()
                    .map(|event| event.modifierFlags().contains(NSEventModifierFlags::Shift))
                    .unwrap_or(false);
                if shift_pressed {
                    return false.into();
                }
                let _ = self.ivars().app.emit(
                    "native-text-area://submitted",
                    SubmittedPayload {
                        field_id: self.ivars().field_id.clone(),
                    },
                );
                true.into()
            }
        }
    );

    impl TextAreaDelegate {
        fn new(
            app: AppHandle,
            field_id: String,
            submit_on_enter: bool,
            mtm: MainThreadMarker,
        ) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(TextAreaDelegateIvars {
                app,
                field_id,
                submit_on_enter: Cell::new(submit_on_enter),
            });
            unsafe { msg_send![super(this), init] }
        }
    }

    struct NativeTextAreaClient {
        scroll_view: Retained<NSScrollView>,
        text_view: Retained<NSTextView>,
        delegate: Retained<TextAreaDelegate>,
        observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    }

    thread_local! {
        static CLIENTS: RefCell<HashMap<String, NativeTextAreaClient>> =
            RefCell::new(HashMap::new());
    }

    pub fn sync(app: &AppHandle, config: NativeTextAreaConfig) {
        let app_for_thread = app.clone();
        let app_for_sync = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            sync_on_main(&app_for_sync, config);
        });
    }

    pub fn remove(app: &AppHandle, field_id: String) {
        let app_for_thread = app.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            remove_on_main(&field_id);
        });
    }

    pub fn focus(app: &AppHandle, field_id: String) {
        let app_for_thread = app.clone();
        let app_for_focus = app_for_thread.clone();
        let _ = app_for_thread.run_on_main_thread(move || {
            focus_on_main(&app_for_focus, &field_id);
        });
    }

    fn sync_on_main(app: &AppHandle, config: NativeTextAreaConfig) {
        let exists = CLIENTS.with(|clients| clients.borrow().contains_key(&config.field_id));
        if !exists && (!config.visible || !create_client(app, &config.field_id)) {
            return;
        }

        let Some(window) = native_window(app) else {
            return;
        };
        let Some(content_view) = window.contentView() else {
            return;
        };
        let bounds = content_view.bounds();
        let frame = frame_for_config(bounds, &config);

        CLIENTS.with(|clients| {
            let clients = clients.borrow();
            let Some(client) = clients.get(&config.field_id) else {
                return;
            };
            let text_view = &client.text_view;
            client
                .delegate
                .ivars()
                .submit_on_enter
                .set(config.submit_on_enter);
            if text_view.string().to_string() != config.value {
                text_view.setString(&NSString::from_str(&config.value));
            }
            // Reapplying editability can interrupt the active AppKit input session.
            if text_view.isEditable() == config.read_only {
                text_view.setEditable(!config.read_only);
            }
            if !text_view.isSelectable() {
                text_view.setSelectable(true);
            }
            client.scroll_view.setFrame(frame);
            let content_size = client.scroll_view.contentSize();
            resize_document_view(text_view, content_size);
            client.scroll_view.setHidden(!config.visible);
            if !config.visible {
                clear_first_responder_if_needed(window, text_view);
            }
        });
    }

    fn create_client(app: &AppHandle, field_id: &str) -> bool {
        let Some(window) = native_window(app) else {
            return false;
        };
        let Some(content_view) = window.contentView() else {
            return false;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };

        let scroll_view = NSScrollView::initWithFrame(
            NSScrollView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        scroll_view.setBorderType(NSBorderType::NoBorder);
        scroll_view.setDrawsBackground(false);
        scroll_view.setHasHorizontalScroller(false);
        scroll_view.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll_view.setAutohidesScrollers(false);
        scroll_view.setHasVerticalScroller(true);
        let scroller = SubtleScroller::new(mtm);
        scroll_view.setVerticalScroller(Some(&scroller));

        let text_view = NSTextView::initWithFrame(
            NSTextView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)),
        );
        text_view.setDrawsBackground(false);
        text_view.setRichText(false);
        text_view.setImportsGraphics(false);
        text_view.setAllowsUndo(true);
        text_view.setEditable(true);
        text_view.setSelectable(true);
        text_view.setFont(Some(&NSFont::systemFontOfSize(12.5)));
        text_view.setTextColor(Some(&NSColor::labelColor()));
        text_view.setTextContainerInset(NSSize::new(8.0, 7.0));
        text_view.setHorizontallyResizable(false);
        text_view.setVerticallyResizable(true);
        text_view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
        let delegate = TextAreaDelegate::new(app.clone(), field_id.to_owned(), false, mtm);
        text_view.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        if let Some(container) = unsafe { text_view.textContainer() } {
            container.setWidthTracksTextView(true);
            container.setContainerSize(NSSize::new(1.0, f64::MAX));
        }
        scroll_view.setDocumentView(Some(&text_view));
        content_view.addSubview_positioned_relativeTo(
            &scroll_view,
            NSWindowOrderingMode::Above,
            None,
        );

        let center = NSNotificationCenter::defaultCenter();
        let app_for_change = app.clone();
        let change_id = field_id.to_owned();
        let text_view_for_change = text_view.clone();
        let change_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_change.emit(
                "native-text-area://changed",
                ChangedPayload {
                    field_id: change_id.clone(),
                    value: text_view_for_change.string().to_string(),
                },
            );
        });
        let change_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSTextDidChangeNotification),
                Some(&text_view),
                None,
                &change_block,
            )
        };

        let app_for_focus = app.clone();
        let focus_id = field_id.to_owned();
        let focus_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_focus.emit(
                "native-text-area://focus-changed",
                FocusPayload {
                    field_id: focus_id.clone(),
                    focused: true,
                },
            );
        });
        let focus_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSTextDidBeginEditingNotification),
                Some(&text_view),
                None,
                &focus_block,
            )
        };

        let app_for_blur = app.clone();
        let blur_id = field_id.to_owned();
        let blur_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            let _ = app_for_blur.emit(
                "native-text-area://focus-changed",
                FocusPayload {
                    field_id: blur_id.clone(),
                    focused: false,
                },
            );
        });
        let blur_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSTextDidEndEditingNotification),
                Some(&text_view),
                None,
                &blur_block,
            )
        };

        CLIENTS.with(|clients| {
            clients.borrow_mut().insert(
                field_id.to_owned(),
                NativeTextAreaClient {
                    scroll_view,
                    text_view,
                    delegate,
                    observers: vec![change_observer, focus_observer, blur_observer],
                },
            );
        });
        true
    }

    fn resize_document_view(text_view: &NSTextView, viewport_size: NSSize) {
        let document_width = viewport_size.width.max(1.0);
        let viewport_height = viewport_size.height.max(1.0);
        text_view.setMinSize(NSSize::new(document_width, viewport_height));
        text_view.setMaxSize(NSSize::new(document_width, f64::MAX));
        text_view.setFrameSize(NSSize::new(document_width, viewport_height));

        let Some(container) = (unsafe { text_view.textContainer() }) else {
            return;
        };
        container.setContainerSize(NSSize::new(document_width, f64::MAX));
        container.setWidthTracksTextView(true);

        let Some(layout_manager) = (unsafe { text_view.layoutManager() }) else {
            return;
        };
        layout_manager.ensureLayoutForTextContainer(&container);
        let used_height = layout_manager
            .usedRectForTextContainer(&container)
            .size
            .height;
        let inset = text_view.textContainerInset();
        let document_height =
            calculated_document_height(viewport_height, used_height, inset.height);
        text_view.setFrameSize(NSSize::new(document_width, document_height));
    }

    fn calculated_document_height(
        viewport_height: f64,
        used_height: f64,
        vertical_inset: f64,
    ) -> f64 {
        (used_height + vertical_inset * 2.0)
            .ceil()
            .max(viewport_height)
    }

    fn frame_for_config(bounds: NSRect, config: &NativeTextAreaConfig) -> NSRect {
        let width = config.width.max(1.0).min(bounds.size.width.max(1.0));
        let height = config.height.max(1.0).min(bounds.size.height.max(1.0));
        let x = config.x.clamp(0.0, (bounds.size.width - width).max(0.0));
        let y = (bounds.size.height - config.y - height)
            .clamp(0.0, (bounds.size.height - height).max(0.0));
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    fn remove_on_main(field_id: &str) {
        let client = CLIENTS.with(|clients| clients.borrow_mut().remove(field_id));
        if let Some(client) = client {
            destroy_client(client);
        }
    }

    fn focus_on_main(app: &AppHandle, field_id: &str) {
        let text_view = CLIENTS.with(|clients| {
            clients
                .borrow()
                .get(field_id)
                .map(|client| client.text_view.clone())
        });
        let (Some(window), Some(text_view)) = (native_window(app), text_view) else {
            return;
        };
        let visible = CLIENTS.with(|clients| {
            clients
                .borrow()
                .get(field_id)
                .map(|client| !client.scroll_view.isHidden())
                .unwrap_or(false)
        });
        if visible && text_view.isEditable() {
            window.makeFirstResponder(Some(&text_view));
        }
    }

    fn destroy_client(client: NativeTextAreaClient) {
        if let Some(window) = client.scroll_view.window() {
            clear_first_responder_if_needed(&window, &client.text_view);
        }
        client.text_view.setDelegate(None);
        let center = NSNotificationCenter::defaultCenter();
        for observer in client.observers {
            unsafe { center.removeObserver(observer.as_ref()) };
        }
        client.scroll_view.removeFromSuperview();
    }

    fn clear_first_responder_if_needed(window: &NSWindow, text_view: &Retained<NSTextView>) {
        let is_first_responder = window
            .firstResponder()
            .map(|responder| {
                Retained::as_ptr(&responder) as *const ()
                    == Retained::as_ptr(text_view) as *const ()
            })
            .unwrap_or(false);
        if is_first_responder {
            window.makeFirstResponder(None);
        }
    }

    fn native_window(app: &AppHandle) -> Option<&NSWindow> {
        app.get_webview_window("main")
            .and_then(|window| window.ns_window().ok())
            .map(|pointer| unsafe { &*pointer.cast::<NSWindow>() })
    }

    #[cfg(test)]
    mod tests {
        use super::{calculated_document_height, scroller_knob_color};

        #[test]
        fn document_height_uses_laid_out_text_and_insets() {
            assert_eq!(calculated_document_height(120.0, 240.2, 7.0), 255.0);
            assert_eq!(calculated_document_height(120.0, 40.0, 7.0), 120.0);
        }

        #[test]
        fn scroller_is_weakly_visible_then_strengthens_on_hover() {
            let default_color = scroller_knob_color(false);
            let hover_color = scroller_knob_color(true);

            assert!(default_color.3 > 0.0);
            assert!(hover_color.3 > default_color.3);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;

    use super::NativeTextAreaConfig;

    pub fn sync(_app: &AppHandle, _config: NativeTextAreaConfig) {}

    pub fn remove(_app: &AppHandle, _field_id: String) {}

    pub fn focus(_app: &AppHandle, _field_id: String) {}
}

pub use imp::{focus, remove, sync};
