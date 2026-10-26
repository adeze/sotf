//! Purposeful views; source tensors remain recorded without generating extra panes.
use rerun::blueprint::{
    Blueprint, ContainerLike, Spatial2DView, Spatial3DView, Tabs, TextDocumentView, TimeSeriesView,
    Vertical,
};

pub(crate) fn blueprint() -> Blueprint {
    let room: ContainerLike = Spatial3DView::new("Room · saved metres, +Z up")
        .with_origin("room")
        .with_contents(["/room/**"])
        .into();
    let measurements: ContainerLike =
        Spatial2DView::new("Measurements · Frequency (Hz, log) / Level (dB)")
            .with_contents(["/measurements/**"])
            .into();
    let live = Vertical::new([
        Spatial2DView::new("Optimizer proposals · Frequency (Hz, log) / Level (dB)")
            .with_contents(["/solver/channels/**"])
            .into(),
        TimeSeriesView::new("Solver history · actual callbacks")
            .with_contents(["/solver/**"])
            .into(),
        TextDocumentView::new("Response snapshot status")
            .with_contents(["/solver/current"])
            .into(),
    ])
    .with_name("Live solver")
    .with_row_shares(vec![0.6, 0.3, 0.1]);
    let final_output = Spatial2DView::new("Final realized output · after safety gates")
        .with_contents(["/solver/final/**"]);
    Blueprint::new(Tabs::new([measurements, room, live.into(), final_output.into()]))
        .with_auto_layout(false)
        .with_auto_views(false)
}
