use super::*;

impl BlazeApp {
    pub(super) fn browser_panel(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Browser");
        ui.add_space(6.0);

        let mut navigate = false;
        let mut go_back = false;
        let mut go_forward = false;
        let mut reload = false;
        let mut open_tab = false;
        ui.horizontal(|ui| {
            go_back = ui.button("←").on_hover_text("Back").clicked();
            go_forward = ui.button("→").on_hover_text("Forward").clicked();
            reload = ui.button("↻").on_hover_text("Reload").clicked();
            let address = ui.add(
                egui::TextEdit::singleline(&mut self.address)
                    .hint_text("Search Google or enter a URL")
                    .desired_width(f32::INFINITY),
            );
            if address.clicked() || address.gained_focus() {
                self.browser_focused = false;
            }
            if address.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                navigate = true;
            }
            if ui.button("Go").clicked() {
                navigate = true;
            }
            if ui.button("☆").on_hover_text("Bookmark current page").clicked() {
                let title = self.tabs.get(self.active_tab).map(|tab| tab.title.clone()).unwrap_or_else(|| "Bookmark".into());
                self.profile.add_bookmark(title, self.address.clone());
                self.toast = "Bookmark added to this profile.".into();
            }
            open_tab = ui.button("+").on_hover_text("New isolated tab").clicked();
        });

        if go_back {
            #[cfg(feature = "servo-browser")]
            if let (Some(browser), Some(Some(page))) = (&self.servo_browser, self.servo_pages.get(self.active_tab)) {
                browser.back(page);
            }
        }
        if go_forward {
            #[cfg(feature = "servo-browser")]
            if let (Some(browser), Some(Some(page))) = (&self.servo_browser, self.servo_pages.get(self.active_tab)) {
                browser.forward(page);
            }
        }
        if reload {
            #[cfg(feature = "servo-browser")]
            if let (Some(browser), Some(Some(page))) = (&self.servo_browser, self.servo_pages.get(self.active_tab)) {
                browser.reload(page);
            }
        }
        if open_tab {
            #[cfg(feature = "servo-browser")]
            if let Some(browser) = self.servo_browser.as_ref() {
                if let Some(Some(old_page)) = self.servo_pages.get(self.active_tab) {
                    browser.deactivate(old_page);
                }
            }
            let id = self.tab_policy.open("https://www.google.com/");
            self.tabs.push(BrowserTab {
                id,
                title: "Google".into(),
                url: "https://www.google.com/".into(),
            });
            #[cfg(feature = "servo-browser")]
            {
                self.servo_pages.push(None);
                self.servo_textures.push(None);
            }
            self.active_tab = self.tabs.len() - 1;
            self.address = "https://www.google.com/".into();
            self.browser_focused = false;
            let suspended = self.tab_policy.enforce_live_limit();
            #[cfg(feature = "servo-browser")]
            self.drop_suspended_pages(&suspended);
        }

        if navigate {
            self.navigate();
        }

        let mut selected_tab = None;
        let mut closed_tab = None;
        ui.horizontal_wrapped(|ui| {
            for (index, tab) in self.tabs.iter().enumerate() {
                let paused = self.tab_policy.entries().iter()
                    .find(|entry| entry.id() == tab.id)
                    .is_some_and(|entry| entry.activity == blaze_core::memory::TabActivity::Suspended);
                let label = if paused { format!("◌ {}", tab.title) } else { tab.title.clone() };
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.active_tab == index, label).clicked() {
                        selected_tab = Some(index);
                    }
                    if ui.small_button("×").on_hover_text("Close tab").clicked() {
                        closed_tab = Some(index);
                    }
                });
            }
        });
        if let Some(index) = closed_tab {
            self.close_browser_tab(index);
        } else if let Some(index) = selected_tab {
            #[cfg(feature = "servo-browser")]
            if let Some(browser) = self.servo_browser.as_ref() {
                if let Some(Some(old_page)) = self.servo_pages.get(self.active_tab) {
                    browser.deactivate(old_page);
                }
            }
            self.active_tab = index;
            self.address = self.tabs[index].url.clone();
            self.browser_focused = false;
            self.tab_policy.select(self.tabs[index].id);
            let suspended = self.tab_policy.enforce_live_limit();
            #[cfg(feature = "servo-browser")]
            self.drop_suspended_pages(&suspended);
            #[cfg(feature = "servo-browser")]
            if let Some(browser) = self.servo_browser.as_ref() {
                if let Some(Some(page)) = self.servo_pages.get(self.active_tab) {
                    browser.activate(page);
                }
            }
        }

        #[cfg(feature = "servo-browser")]
        {
            let scale = ui.ctx().pixels_per_point();
            let width = ui.available_width().max(1.0);
            let height = ui.available_height().max(260.0);
            let display_size = egui::vec2(width, height);
            let (rect, response) = ui.allocate_exact_size(display_size, egui::Sense::click_and_drag());
            if response.clicked() {
                self.browser_focused = true;
                response.request_focus();
            }

            if self.servo_pages.len() < self.tabs.len() {
                self.servo_pages.resize_with(self.tabs.len(), || None);
            }
            if self.servo_textures.len() < self.tabs.len() {
                self.servo_textures.resize_with(self.tabs.len(), || None);
            }
            if let Some(browser) = self.servo_browser.as_ref() {
                let mut created_page = false;
                if self.servo_pages[self.active_tab].is_none() {
                    match browser.create_page(&self.tabs[self.active_tab].url, scale) {
                        Ok(page) => {
                            self.servo_pages[self.active_tab] = Some(page);
                            created_page = true;
                        }
                        Err(error) => self.browser_status = format!("Could not create browser tab: {error}"),
                    }
                }

                if let Some(page) = self.servo_pages[self.active_tab].as_ref() {
                    if created_page {
                        browser.activate(page);
                    }
                    let events = ui.input(|input| input.raw.events.clone());
                    for event in events {
                        match event {
                            egui::Event::PointerMoved(position) => {
                                if rect.contains(position) {
                                    browser.forward_mouse_move(page, (position.x - rect.left()) * scale, (position.y - rect.top()) * scale);
                                }
                            }
                            egui::Event::PointerButton { pos, button, pressed, .. } => {
                                if rect.contains(pos) || (!pressed && self.browser_focused) {
                                    browser.forward_mouse_button(page, (pos.x - rect.left()) * scale, (pos.y - rect.top()) * scale, button, pressed);
                                }
                            }
                            egui::Event::MouseWheel { delta, .. } if response.hovered() => {
                                let pointer = ui.input(|input| input.pointer.hover_pos()).unwrap_or(rect.center());
                                browser.forward_wheel(page, (pointer.x - rect.left()) * scale, (pointer.y - rect.top()) * scale, delta.x * scale, delta.y * scale);
                            }
                            egui::Event::Key { key, pressed, modifiers, .. } if self.browser_focused => {
                                if let Some(key) = crate::servo_embed::ServoBrowser::key_from_egui(key) {
                                    browser.forward_key(page, key, pressed, modifiers);
                                }
                            }
                            egui::Event::Text(text) if self.browser_focused => {
                                let modifiers = ui.input(|input| input.modifiers);
                                for character in text.chars() {
                                    browser.forward_key(page, servo::Key::Character(character.to_string()), true, modifiers);
                                    browser.forward_key(page, servo::Key::Character(character.to_string()), false, modifiers);
                                }
                            }
                            _ => {}
                        }
                    }

                    let physical_size = winit::dpi::PhysicalSize::new(
                        (rect.width() * scale).max(1.0) as u32,
                        (rect.height() * scale).max(1.0) as u32,
                    );
                    match browser.render(page, physical_size) {
                        Ok(Some(image)) => {
                            if let Some(texture) = self.servo_textures[self.active_tab].as_mut() {
                                texture.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                self.servo_textures[self.active_tab] = Some(ui.ctx().load_texture(
                                    format!("blaze-servo-page-{}", self.tabs[self.active_tab].id.get()),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.browser_status = format!("Browser rendering error: {error}");
                        }
                    }

                    self.address = browser.current_url(page);
                    self.tabs[self.active_tab].url = self.address.clone();
                    self.tabs[self.active_tab].title = browser.title(page);
                }
            }

            if let Some(texture) = self.servo_textures.get(self.active_tab).and_then(Option::as_ref) {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
                    egui::Color32::WHITE,
                );
            } else {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if self.browser_status.is_empty() { "Starting Servo…" } else { &self.browser_status },
                    egui::FontId::proportional(14.0),
                    ui.visuals().text_color(),
                );
            }
        }

        #[cfg(not(feature = "servo-browser"))]
        {
            ui.label("Build Blaze with --features servo-browser to enable the embedded Servo engine.");
        }

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Tabs").strong());
            ui.label(format!(
                "{} open · {} MiB soft memory budget",
                self.tabs.len(),
                self.tab_policy.budget().soft_limit_mib()
            ));
            if !self.browser_status.is_empty() {
                ui.label(&self.browser_status);
            }
        });
    }

    pub(super) fn close_browser_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        if self.tabs.len() == 1 {
            let home = "https://www.google.com/";
            self.tabs[0].title = "Google".into();
            self.tabs[0].url = home.into();
            self.address = home.into();
            #[cfg(feature = "servo-browser")]
            if let (Some(browser), Some(Some(page))) = (&self.servo_browser, self.servo_pages.first()) {
                let _ = browser.navigate(page, home);
            }
            return;
        }

        #[cfg(feature = "servo-browser")]
        if let Some(browser) = self.servo_browser.as_ref() {
            if index == self.active_tab {
                if let Some(Some(page)) = self.servo_pages.get(index) {
                    browser.deactivate(page);
                }
            }
        }

        let id = self.tabs[index].id;
        self.tab_policy.close(id);
        self.tabs.remove(index);
        #[cfg(feature = "servo-browser")]
        {
            self.servo_pages.remove(index);
            self.servo_textures.remove(index);
        }

        if index < self.active_tab {
            self.active_tab -= 1;
        } else if index == self.active_tab {
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }

        let selected = self.tabs[self.active_tab].clone();
        self.address = selected.url;
        self.tab_policy.select(selected.id);
        #[cfg(feature = "servo-browser")]
        if let Some(browser) = self.servo_browser.as_ref() {
            if let Some(Some(page)) = self.servo_pages.get(self.active_tab) {
                browser.activate(page);
            }
        }
    }

    #[cfg(feature = "servo-browser")]
    pub(super) fn drop_suspended_pages(&mut self, ids: &[TabId]) {
        for id in ids {
            if let Some(index) = self.tabs.iter().position(|tab| tab.id == *id) {
                if index != self.active_tab {
                    self.servo_pages[index] = None;
                    self.servo_textures[index] = None;
                }
            }
        }
    }

    pub(super) fn navigate(&mut self) {
        match normalize_address(&self.address) {
            Ok(url) => {
                let normalized = url.to_string();
                self.address = normalized.clone();
                let title = url.host_str().unwrap_or("Blank").to_owned();
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.url = normalized.clone();
                    tab.title = title.clone();
                    self.tab_policy.set_url(tab.id, normalized.clone());
                }
                self.profile.record_visit(title, normalized.clone(), unix_now(), 500);
                #[cfg(feature = "servo-browser")]
                if let (Some(browser), Some(Some(page))) = (&self.servo_browser, self.servo_pages.get(self.active_tab)) {
                    if let Err(error) = browser.navigate(page, &normalized) {
                        self.browser_status = format!("Navigation failed: {error}");
                    }
                }
                self.browser_status = format!("Navigated to {normalized}");
            }
            Err(error) => {
                self.browser_status = format!("Navigation rejected: {error}");
            }
        }
    }

}
