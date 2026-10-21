use sotf_audio_player::ReleaseChannel;

use super::Screen;

impl Screen {
    /// Returns the maturity level of this screen.
    pub fn maturity(&self) -> ReleaseChannel {
        match self {
            #[cfg(feature = "speaker-workbench")]
            Screen::SpeakerWorkbench => ReleaseChannel::Prod,
            Screen::Home
            | Screen::HomeShelf
            | Screen::Library
            | Screen::NowPlaying
            | Screen::Queue
            | Screen::Spectrum
            | Screen::Settings
            | Screen::SettingsDetail
            | Screen::StudioHub
            | Screen::EqCurve
            | Screen::Recording
            | Screen::HeadphoneEq
            | Screen::Spinorama
            | Screen::ListeningTest
            | Screen::Studio
            | Screen::RoomEq
            | Screen::Streams
            | Screen::PluginGraph
            | Screen::Playlists => ReleaseChannel::Prod,
        }
    }
}
