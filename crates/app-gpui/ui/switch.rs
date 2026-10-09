impl PlayerView {
    pub(crate) fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(screen, "action", cx);
    }

    pub(crate) fn switch_screen_with_trigger(
        &mut self,
        screen: Screen,
        trigger: &str,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            let leaving_ear_training = state.app.ui_state.current_screen == Screen::ListeningTest
                && screen != Screen::ListeningTest;
            if leaving_ear_training {
                use crate::components::plugins::editing::PluginEditingManager;
                let _ = state.app.plugin_state.leave_ab_test_runtime();
                let owned_node = state
                    .app
                    .plugin_state
                    .listening_test_state
                    .eq_audition_node_id
                    .take();
                if let Some(index) = owned_node
                    .and_then(|node_id| state.app.plugin_state.graph.linear_index_of_node(node_id))
                {
                    state.app.remove_plugin(index);
                }
                state.app.plugin_state.listening_test_state.eq_filtered = false;
            }
            state.app.set_screen(screen, trigger);
        });
        cx.notify();
    }

    // Screen changes unmount focused controls, including plugin dropdowns.
    // Give subsequent keyboard/menu actions a live root dispatch target.
    fn switch_to_library(
        &mut self,
        _: &SwitchToLibrary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Library, "SwitchToLibrary", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_home(&mut self, _: &SwitchToHome, window: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Home, "SwitchToHome", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_now_playing(
        &mut self,
        _: &SwitchToNowPlaying,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::NowPlaying, "SwitchToNowPlaying", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_queue(&mut self, _: &SwitchToQueue, window: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen_with_trigger(Screen::Queue, "SwitchToQueue", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_playlists(
        &mut self,
        _: &SwitchToPlaylists,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Playlists, "SwitchToPlaylists", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_plugins(
        &mut self,
        _: &SwitchToPlugins,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Settings, "SwitchToPlugins", cx);
        self.focus_handle.focus(window, cx);
        self.state.update(cx, |state, _cx| {
            state.app.ui_state.active_settings_tab = crate::app::SettingsTab::Misc;
        });
        cx.notify();
    }

    fn switch_to_studio(
        &mut self,
        _: &SwitchToStudio,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Studio, "SwitchToStudio", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_plugin_graph(
        &mut self,
        _: &SwitchToPluginGraph,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::PluginGraph, "SwitchToPluginGraph", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_listening_test(
        &mut self,
        _: &SwitchToListeningTest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::ListeningTest, "SwitchToListeningTest", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_spinorama(
        &mut self,
        _: &SwitchToSpinorama,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Spinorama, "SwitchToSpinorama", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_devices(
        &mut self,
        _: &SwitchToDevices,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Settings, "SwitchToDevices", cx);
        self.focus_handle.focus(window, cx);
        self.state.update(cx, |state, _cx| {
            state.app.ui_state.active_settings_tab = crate::app::SettingsTab::AudioDevice;
        });
        cx.notify();
    }

    fn switch_to_directories(
        &mut self,
        _: &SwitchToDirectories,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Settings, "SwitchToDirectories", cx);
        self.focus_handle.focus(window, cx);
        self.state.update(cx, |state, _cx| {
            state.app.ui_state.active_settings_tab = crate::app::SettingsTab::Library;
        });
        cx.notify();
    }

    fn switch_to_settings(
        &mut self,
        _: &SwitchToSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Settings, "SwitchToSettings", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_recording(
        &mut self,
        _: &SwitchToRecording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Recording, "SwitchToRecording", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_room_eq(
        &mut self,
        _: &SwitchToRoomEQ,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::RoomEq, "SwitchToRoomEQ", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_headphone_eq(
        &mut self,
        _: &SwitchToHeadphoneEQ,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::HeadphoneEq, "SwitchToHeadphoneEQ", cx);
        self.focus_handle.focus(window, cx);
    }

    fn switch_to_spectrum(
        &mut self,
        _: &SwitchToSpectrum,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen_with_trigger(Screen::Spectrum, "SwitchToSpectrum", cx);
        self.focus_handle.focus(window, cx);
    }
}
