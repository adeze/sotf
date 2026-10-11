//! Native window hosting the shared AutoEQ report and its WASM renderer.

// Rust guideline compliant 2026-02-21

use super::PlayerView;
use gpui::{prelude::*, *};
use sotf_report_host::{GeneratedReport, ReportServer};

impl PlayerView {
    pub(crate) fn open_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(false, cx);
    }

    pub(crate) fn save_shared_room_eq_report(&mut self, cx: &mut Context<Self>) {
        self.prepare_shared_room_eq_report(true, cx);
    }

    fn prepare_shared_room_eq_report(&mut self, save: bool, cx: &mut Context<Self>) {
        let (result, artifacts, dark, inputs, labels) = {
            let app = &self.state.read(cx).app;
            let room = &app.measurement_state.room_eq_state;
            let labels = crate::app::i18n::SharedReportText::for_language(app.ui_state.language);
            if !room.result_is_current() {
                self.report_error(labels.stale.into(), cx);
                return;
            }
            let Some(result) = room.dsp_output.as_ref() else {
                return;
            };
            let result = match serde_json::to_value(result) {
                Ok(value) => value,
                Err(error) => {
                    self.report_error(error.to_string(), cx);
                    return;
                }
            };
            (
                result,
                room.artifact_dir.clone(),
                app.ui_state.theme_id != crate::app::theme::ThemeId::Light,
                room.result_inputs.clone(),
                labels,
            )
        };
        self.state.update(cx, |state, _| {
            state.app.measurement_state.room_eq_state.status_message = labels.preparing.into();
        });
        let worker = cx.background_executor().spawn(async move {
            sotf_report_host::generate_report(&result, artifacts.as_deref(), None)
        });
        cx.spawn(async move |view, cx| match worker.await {
            Ok(report) => {
                let current = view
                    .update(cx, |view, cx| {
                        let room = &view.state.read(cx).app.measurement_state.room_eq_state;
                        room.result_is_current() && room.result_inputs == inputs
                    })
                    .unwrap_or(false);
                if !current {
                    let _ = view.update(cx, |view, cx| view.report_error(labels.stale.into(), cx));
                    return;
                }
                if save {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("AutoEQ HTML", &["html"])
                        .set_title(labels.save)
                        .set_file_name("roomeq-report.html")
                        .save_file()
                        .await;
                    if let Some(file) = file
                        && let Err(error) = report.save(file.path())
                    {
                        let _ =
                            view.update(cx, |view, cx| view.report_error(error.to_string(), cx));
                    }
                    return;
                }
                if let Err(error) = open_report_window(report, dark, cx) {
                    let _ = view.update(cx, |view, cx| view.report_error(error.to_string(), cx));
                }
            }
            Err(error) => {
                let _ = view.update(cx, |view, cx| {
                    view.report_error(format!("Cannot open AutoEQ report: {error:#}"), cx)
                });
            }
        })
        .detach();
    }

    fn report_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            state.app.measurement_state.room_eq_state.error_message = Some(error.clone());
            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(error));
        });
        cx.notify();
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
fn open_report_window(
    report: GeneratedReport,
    dark: bool,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let server = ReportServer::start(report.read_html()?)?;
    cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("RoomEQ — AutoEQ report".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| SharedReportWindow::new(report, server, dark, window, cx)),
    )?;
    Ok(())
}

#[cfg(not(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
)))]
fn open_report_window(
    _report: GeneratedReport,
    _dark: bool,
    _cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    anyhow::bail!("Embedded reports require the report-webview feature on macOS or Windows")
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
struct SharedReportWindow {
    // Drop the child view before stopping its origin or deleting report files.
    webview: Option<wry::WebView>,
    _server: ReportServer,
    _report: GeneratedReport,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
impl SharedReportWindow {
    fn new(
        report: GeneratedReport,
        server: ReportServer,
        dark: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let url = server.url().to_owned();
        let navigation_url = url.clone();
        let report_id = server.report_id().to_owned();
        let builder = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_bounds(webview_bounds(window))
            .with_initialization_script(sotf_report_host::initialization_script(&report_id, dark))
            .with_navigation_handler(move |candidate| {
                candidate.split('#').next() == Some(navigation_url.as_str())
            })
            .with_ipc_handler(move |request| {
                match sotf_report_host::parse_message(request.body(), &report_id) {
                    Ok(action) => log::debug!("AutoEQ report interaction: {action:?}"),
                    Err(error) => log::warn!("Rejected AutoEQ report interaction: {error}"),
                }
            });
        let (webview, error) = match builder.build_as_child(&*window) {
            Ok(view) => (Some(view), None),
            Err(error) => (
                None,
                Some(format!("Cannot create shared report view: {error}")),
            ),
        };
        let subscription = cx.observe_window_bounds(window, |this, window, cx| {
            if let Some(view) = &this.webview
                && let Err(error) = view.set_bounds(webview_bounds(window))
            {
                this.error = Some(error.to_string());
                cx.notify();
            }
        });
        Self {
            webview,
            _server: server,
            _report: report,
            error,
            _subscriptions: vec![subscription],
        }
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
fn webview_bounds(window: &Window) -> wry::Rect {
    let size = window.viewport_size();
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
        size: wry::dpi::LogicalSize::new(
            f64::from(f32::from(size.width)),
            f64::from(f32::from(size.height)),
        )
        .into(),
    }
}

#[cfg(all(
    feature = "report-webview",
    any(target_os = "macos", target_os = "windows")
))]
impl Render for SharedReportWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .when_some(self.error.as_ref(), |view, error| view.child(error.clone()))
    }
}
