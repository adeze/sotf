//! Inline native child for the owned, loopback-only Rerun viewer.
use super::PlayerView;
use crate::app::{InputMode, Screen};
use crate::components::design::Ds;
use gpui::{prelude::*, *};
use gpui_ui_kit::{Button, ButtonSize, ButtonVariant, Text};
use std::{path::PathBuf, sync::Arc};

#[path = "scientific_viewer_policy.rs"]
mod policy;

fn viewer_cache_directory() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Caches"));
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")));
    base.map(|path| path.join("sotf/rerun"))
        .ok_or_else(|| "Cannot determine the application cache directory".into())
}

impl PlayerView {
    pub(crate) fn open_scientific_viewer(&mut self, cx: &mut Context<Self>) {
        self.scientific_viewer_open = true;
        if self.scientific_viewer_session.is_some() {
            self.scientific_viewer_error = self.publish_scientific_viewer_data(cx).err();
            cx.notify();
            return;
        }
        if self.scientific_viewer_starting {
            return;
        }
        let output = self
            .state
            .read(cx)
            .app
            .measurement_state
            .room_eq_state
            .artifact_dir
            .clone()
            .map(Ok)
            .unwrap_or_else(viewer_cache_directory);
        let output = match output {
            Ok(path) => path,
            Err(error) => {
                self.scientific_viewer_error = Some(error);
                cx.notify();
                return;
            }
        };
        self.scientific_viewer_starting = true;
        self.scientific_viewer_error = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let session = cx
                .background_executor()
                .spawn(async move { sotf_rerun_viewer::ViewerSession::new(&output).map(Arc::new) })
                .await;
            let _ = view.update(cx, |view, cx| {
                view.scientific_viewer_starting = false;
                match session {
                    Ok(session) => {
                        view.scientific_viewer_session = Some(Arc::clone(&session));
                        view.scientific_viewer_panel =
                            Some(cx.new(|_| ScientificViewerPanel::new(session)));
                        view.scientific_viewer_error =
                            view.publish_scientific_viewer_data(cx).err();
                    }
                    Err(error) => view.scientific_viewer_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_scientific_viewer_room(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(any(target_os = "ios", target_os = "tvos")))]
        cx.spawn(async move |view, cx| {
            let file = rfd::AsyncFileDialog::new()
                .set_title("Choose registered room model")
                .add_filter("Room model JSON", &["json"])
                .pick_file()
                .await;
            if let Some(file) = file {
                let _ = view.update(cx, |view, cx| {
                    view.scientific_viewer_room_path = Some(file.path().to_path_buf());
                    view.scientific_viewer_error = view.publish_scientific_viewer_data(cx).err();
                    cx.notify();
                });
            }
        })
        .detach();
        #[cfg(any(target_os = "ios", target_os = "tvos"))]
        {
            self.scientific_viewer_error =
                Some("Room file selection is unavailable on this platform.".into());
            cx.notify();
        }
    }

    /// Native child views do not participate in GPUI's clipping/overlay stack.
    pub(crate) fn prepare_scientific_viewer_frame(&self, cx: &mut Context<Self>) {
        if let Some(panel) = &self.scientific_viewer_panel {
            let state = self.state.read(cx);
            let allowed = self.scientific_viewer_open
                && state.app.ui_state.current_screen == Screen::RoomEq
                && state.app.ui_state.input_mode == InputMode::Normal
                && state.app.ui_state.active_menu == crate::app::ActiveMenu::None
                && state.app.ui_state.context_menu.is_none();
            panel.update(cx, |panel, _| {
                panel.allowed = allowed;
                // A clipped/cull-skipped canvas must not leave a native view over the shell.
                panel.hide();
            });
        }
    }

    pub(crate) fn render_scientific_viewer_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let open_view = view.clone();
        let refresh_view = view.clone();
        let room_view = view;
        let open = self.scientific_viewer_open;
        let ready = self.scientific_viewer_session.is_some();
        let mut controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(Ds::from_cx(cx).gap_md)
            .child(
                Button::new(
                    "scientific-viewer-toggle",
                    if open {
                        "Back to workflow"
                    } else {
                        "Rerun view"
                    },
                )
                .size(ButtonSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, cx| {
                    open_view.update(cx, |view, cx| {
                        if view.scientific_viewer_open {
                            view.scientific_viewer_open = false;
                            cx.notify();
                        } else {
                            view.open_scientific_viewer(cx);
                        }
                    })
                }),
            );
        if open {
            controls = controls
                .child(
                    Button::new("scientific-viewer-refresh", "Refresh measurements")
                        .size(ButtonSize::Sm)
                        .disabled(!ready)
                        .on_click(move |_, cx| {
                            refresh_view.update(cx, |view, cx| {
                                view.scientific_viewer_error =
                                    view.publish_scientific_viewer_data(cx).err();
                                cx.notify();
                            })
                        }),
                )
                .child(
                    Button::new("scientific-viewer-room", "Choose room…")
                        .size(ButtonSize::Sm)
                        .disabled(!ready)
                        .on_click(move |_, cx| {
                            room_view.update(cx, |view, cx| {
                                view.choose_scientific_viewer_room(cx);
                            })
                        }),
                );
        }
        controls.into_any_element()
    }

    pub(crate) fn render_scientific_viewer(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .flex()
            .flex_col()
            .gap(Ds::from_cx(cx).gap_md)
            .min_w_0();
        if let Some(error) = &self.scientific_viewer_error {
            content = content.child(Text::body(error.clone()));
        }
        if self.scientific_viewer_starting {
            content = content.child(Text::body("Starting Rerun…"));
        } else if let Some(panel) = &self.scientific_viewer_panel {
            content = content.child(panel.clone());
        } else {
            content = content.child(Text::body(
                "Scientific viewer unavailable. Return to the workflow and retry.",
            ));
        }
        content.into_any_element()
    }
}

pub(crate) struct ScientificViewerPanel {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    webview: Option<wry::WebView>,
    _session: Arc<sotf_rerun_viewer::ViewerSession>,
    url: String,
    error: Option<String>,
    allowed: bool,
}

impl ScientificViewerPanel {
    fn new(session: Arc<sotf_rerun_viewer::ViewerSession>) -> Self {
        let url = session.viewer_url().to_owned();
        // WKWebView's WebGPU path failed Metal shader compilation during native QA.
        #[cfg(target_os = "macos")]
        let url = format!("{url}&renderer=webgl");
        Self {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            webview: None,
            _session: session,
            url,
            error: None,
            allowed: false,
        }
    }

    fn hide(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if let Some(view) = &self.webview
            && let Err(error) = view.set_visible(false)
        {
            self.error = Some(error.to_string());
            self.webview = None;
        }
    }

    fn place(&mut self, bounds: Bounds<Pixels>, window: &Window) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if !self.allowed
                || self.error.is_some()
                || bounds.size.width <= Pixels::ZERO
                || bounds.size.height <= Pixels::ZERO
            {
                self.hide();
                return;
            }
            let rect = wry::Rect {
                position: wry::dpi::LogicalPosition::new(
                    f64::from(f32::from(bounds.origin.x)),
                    f64::from(f32::from(bounds.origin.y)),
                )
                .into(),
                size: wry::dpi::LogicalSize::new(
                    f64::from(f32::from(bounds.size.width)),
                    f64::from(f32::from(bounds.size.height)),
                )
                .into(),
            };
            if self.webview.is_none() {
                let navigation_url = self.url.clone();
                match wry::WebViewBuilder::new()
                    .with_url(&self.url)
                    .with_bounds(rect)
                    .with_navigation_handler(move |candidate| {
                        policy::navigation_allowed(&navigation_url, &candidate)
                    })
                    .build_as_child(window)
                {
                    Ok(view) => self.webview = Some(view),
                    Err(error) => self.error = Some(format!("Cannot embed Rerun: {error}")),
                }
            }
            if let Some(view) = &self.webview {
                if let Err(error) = view.set_bounds(rect).and_then(|_| view.set_visible(true)) {
                    self.error = Some(error.to_string());
                    self.webview = None;
                }
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (bounds, window);
            self.error = Some("Inline Rerun is supported on macOS and Windows.".into());
        }
    }
}

impl Render for ScientificViewerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let mut content = div().flex().flex_col().w_full().min_w_0();
        if let Some(error) = &self.error {
            content = content.child(Text::body(error.clone()));
        }
        content.child(
            canvas(
                move |bounds, window, cx| {
                    let bounds = bounds.intersect(&window.content_mask().bounds);
                    entity.update(cx, |view, cx| {
                        let previous = view.error.clone();
                        view.place(bounds, window);
                        if view.error != previous {
                            cx.notify();
                        }
                    });
                },
                |_, _, _, _| {},
            )
            .w_full()
            .h(rems(34.0)),
        )
    }
}
