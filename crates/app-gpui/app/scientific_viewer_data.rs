//! Shared-state inputs for the scientific viewer; no correction or geometry inference.
use crate::ui::PlayerView;
use gpui::Context;
use sotf_rerun_viewer::room::RoomGeometry;

impl PlayerView {
    pub(crate) fn publish_scientific_viewer_data(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let session = self
            .scientific_viewer_session
            .clone()
            .ok_or("Scientific viewer is not running")?;
        session.clear_inputs()?;
        let mut errors = Vec::new();
        {
            let state = self.state.read(cx);
            let channels = &state
                .app
                .measurement_state
                .room_eq_state
                .model
                .channel_measurements;
            for channel in channels {
                let takes = std::iter::once(("Primary".to_owned(), &channel.measurement))
                    .chain(
                        channel
                            .multi_mic_measurements
                            .iter()
                            .enumerate()
                            .map(|(i, take)| (format!("Additional mic {}", i + 1), take)),
                    )
                    .chain(
                        channel
                            .group_drivers
                            .iter()
                            .enumerate()
                            .map(|(i, take)| (format!("Driver {}", i + 1), take)),
                    );
                for (role, take) in takes {
                    let label = format!("{} · {role}", channel.channel_name);
                    let hz: Vec<f64> = take.frequencies.iter().map(|&v| f64::from(v)).collect();
                    let db: Vec<f64> = take.magnitude_db.iter().map(|&v| f64::from(v)).collect();
                    if let Err(error) = session.log_measurement(&label, &hz, &db) {
                        errors.push(format!("{label}: {error}"));
                    }
                }
            }
        }
        if let Some(path) = &self.scientific_viewer_room_path {
            match RoomGeometry::load_model(path) {
                Ok(room) => {
                    session.log_room_mesh(&room.vertices, &room.indices, &room.provenance)?
                }
                Err(error) => errors.push(error),
            }
        }
        session.flush()?;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}
