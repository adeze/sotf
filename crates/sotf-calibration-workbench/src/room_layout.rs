//! Channel-role schematic only; measurement names do not establish physical positions.
use gpui::prelude::*;
use gpui::*;
use gpui_ui_kit::{Text, ThemeExt};

#[derive(Debug, PartialEq)]
enum Placement {
    Floor(usize),
    Height,
    Unpositioned,
}

fn placement(name: &str) -> Placement {
    match name.trim().to_ascii_uppercase().as_str() {
        "L" | "FL" | "FRONT LEFT" => Placement::Floor(0),
        "C" | "FC" | "CENTER" | "CENTRE" => Placement::Floor(1),
        "R" | "FR" | "FRONT RIGHT" => Placement::Floor(2),
        "SL" | "SLA" | "LS" | "SURROUND LEFT" => Placement::Floor(3),
        "SR" | "SRA" | "RS" | "SURROUND RIGHT" => Placement::Floor(5),
        "BL" | "SBL" | "RL" | "REAR LEFT" => Placement::Floor(6),
        "BC" | "SBC" | "BACK CENTER" => Placement::Floor(7),
        "BR" | "SBR" | "RR" | "REAR RIGHT" => Placement::Floor(8),
        "FHL" | "FHR" | "TFL" | "TFR" | "TSL" | "TSR" | "TBL" | "TBR" | "TFC" | "TBC" | "TSC"
        | "VOG" | "RHL" | "RHR" => Placement::Height,
        _ => Placement::Unpositioned,
    }
}

pub fn render_channel_layout(channels: &[String], cx: &App) -> AnyElement {
    let theme = cx.theme();
    let mut grid = div().flex().flex_col().gap_2().w_full();
    for row in 0..3 {
        let mut strip = div().flex().gap_2().w_full();
        for column in 0..3 {
            let cell = row * 3 + column;
            let mut contents = div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .flex_1()
                .min_w_0()
                .min_h(rems(3.0));
            if cell == 4 {
                contents =
                    contents.child(Text::caption("Listener reference").color(theme.text_secondary));
            }
            for channel in channels
                .iter()
                .filter(|name| placement(name) == Placement::Floor(cell))
            {
                contents = contents.child(
                    div()
                        .rounded_md()
                        .bg(theme.accent_muted)
                        .px_2()
                        .py_1()
                        .child(Text::label(channel.clone()).color(theme.text_primary)),
                );
            }
            strip = strip.child(contents);
        }
        grid = grid.child(strip);
    }
    let height = channels
        .iter()
        .filter(|name| placement(name) == Placement::Height)
        .cloned()
        .collect::<Vec<_>>();
    let unpositioned = channels
        .iter()
        .filter(|name| placement(name) == Placement::Unpositioned)
        .cloned()
        .collect::<Vec<_>>();
    let mut panel = div()
        .flex()
        .flex_col()
        .gap_2()
        .min_w_0()
        .child(Text::section_header("Channel layout · schematic"))
        .child(Text::caption("Front / screen").color(theme.text_secondary))
        .child(grid)
        .child(Text::caption("Rear").color(theme.text_secondary));
    if channels.is_empty() {
        panel = panel.child(Text::body("Load measurements to show their channel roles."));
    }
    if !height.is_empty() {
        panel = panel.child(Text::label(format!(
            "Height channels: {}",
            height.join(", ")
        )));
    }
    if !unpositioned.is_empty() {
        panel = panel.child(Text::label(format!(
            "Position unspecified: {}",
            unpositioned.join(", ")
        )));
    }
    panel.child(Text::caption("Role arrangement only. Room dimensions, speaker coordinates and soundstage predictions are unavailable in this view.").color(theme.text_secondary))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Placement, placement};

    #[test]
    fn channel_aliases_keep_left_right_and_height_distinct() {
        for (name, cell) in [
            ("FL", 0),
            ("C", 1),
            ("FR", 2),
            ("SL", 3),
            ("SRA", 5),
            ("SBL", 6),
            ("SBR", 8),
        ] {
            assert_eq!(placement(name), Placement::Floor(cell));
        }
        for name in ["FHL", "FHR", "TFL", "TBR", "VOG"] {
            assert_eq!(placement(name), Placement::Height);
        }
        for name in ["SW1", "SUB", "", "Measured speaker 2"] {
            assert_eq!(placement(name), Placement::Unpositioned);
        }
        assert_eq!(placement(" front left "), Placement::Floor(0));
    }
}
