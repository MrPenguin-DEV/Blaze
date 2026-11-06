use blaze_browser::normalize_address;
use blaze_core::{
    memory::MemoryBudget,
    tabs::TabManager,
    TabId,
};
use blaze_vault::{read_encrypted, write_encrypted};
use directories::ProjectDirs;
use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Section {
    Browser,
    Code,
    Documents,
    Sheets,
    Slides,
    Notes,
    Messenger,
    Calling,
    Vault,
}

impl Section {
    const ALL: [(Self, &'static str); 9] = [
        (Self::Browser, "Browser"),
        (Self::Code, "Code Studio"),
        (Self::Documents, "Documents"),
        (Self::Sheets, "Sheets"),
        (Self::Slides, "Presentations"),
        (Self::Notes, "Notes"),
        (Self::Messenger, "Messenger"),
        (Self::Calling, "Calling"),
        (Self::Vault, "Vault"),
    ];
}

#[derive(Clone, Debug)]
struct BrowserTab {
    id: TabId,
    title: String,
    url: String,
}

pub struct BlazeApp {
    section: Section,
    tabs: Vec<BrowserTab>,
    tab_policy: TabManager,
    active_tab: usize,
    address: String,
    browser_status: String,
    code_name: String,
    code_source: String,
    document_title: String,
    document_markdown: String,
    sheet_title: String,
    sheet_csv: String,
    presentation_title: String,
    slide_outline: String,
    notes: String,
    message_to: String,
    message_body: String,
    call_status: String,
    vault_password: String,
    vault_status: String,
    toast: String,
}

impl Default for BlazeApp {
    fn default() -> Self {
        let mut tab_policy = TabManager::new(MemoryBudget::new(1024), 8);
        let initial_tab = tab_policy.open("about:blank");
        Self {
            section: Section::Browser,
            tabs: vec![BrowserTab {
                id: initial_tab,
                title: "New tab".into(),
                url: "about:blank".into(),
            }],
            tab_policy,
            active_tab: 0,
            address: "https://example.com".into(),
            browser_status: "Browser engine not connected yet. The native shell is ready.".into(),
            code_name: "scratch.rs".into(),
            code_source: "fn main() {\n    println!(\"Hello from Blaze!\");\n}\n".into(),
            document_title: "Untitled document".into(),
            document_markdown: "# Untitled document\n\nStart writing here.\n".into(),
            sheet_title: "Untitled spreadsheet".into(),
            sheet_csv: "Name,Value\nBlaze,1\n".into(),
            presentation_title: "Untitled presentation".into(),
            slide_outline: "# Slide 1 — Welcome\n\nBlaze is a Rust-first workspace.\n\n---\n\n# Slide 2 — Roadmap\n\nAdd your next slide here.\n".into(),
            notes: String::new(),
            message_to: String::new(),
            message_body: String::new(),
            call_status: "Calling transport is not configured. No microphone or camera is accessed.".into(),
            vault_password: String::new(),
            vault_status: "Vault is locked. Enter a strong passphrase to load or save encrypted notes.".into(),
            toast: String::new(),
        }
    }
}

impl eframe::App for BlazeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.render_header(&ctx);
        self.render_sidebar(&ctx);
        egui::CentralPanel::default().show(&ctx, |ui| {
            if !self.toast.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&self.toast).color(egui::Color32::LIGHT_GREEN));
                    if ui.small_button("Dismiss").clicked() {
                        self.toast.clear();
                    }
                });
                ui.add_space(8.0);
            }

            match self.section {
                Section::Browser => self.browser_panel(ui),
                Section::Code => self.code_panel(ui),
                Section::Documents => self.documents_panel(ui),
                Section::Sheets => self.sheets_panel(ui),
                Section::Slides => self.slides_panel(ui),
                Section::Notes => self.notes_panel(ui),
                Section::Messenger => self.messenger_panel(ui),
                Section::Calling => self.calling_panel(ui),
                Section::Vault => self.vault_panel(ui),
            }
        });
    }
}

impl BlazeApp {
    fn render_header(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("blaze_header")
            .exact_height(58.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    ui.heading(egui::RichText::new("BLAZE").strong().size(22.0));
                    ui.label(egui::RichText::new("RUST WORKSPACE").small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("NATIVE · GPU UI · LOCAL-FIRST").small().weak());
                        ui.add_space(12.0);
                    });
                });
            });
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("blaze_sidebar")
            .resizable(false)
            .default_width(178.0)
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("WORKSPACE").small().weak().strong());
                ui.add_space(8.0);
                for (section, label) in Section::ALL {
                    let selected = self.section == section;
                    if ui
                        .selectable_label(selected, egui::RichText::new(label).size(14.0))
                        .clicked()
                    {
                        self.section = section;
                    }
                }
                ui.add_space(20.0);
                ui.separator();
                ui.label(egui::RichText::new("STATUS").small().weak().strong());
                ui.label(egui::RichText::new("Engine: disconnected").small());
                ui.label(egui::RichText::new("Vault: password locked").small());
                ui.label(egui::RichText::new("Network: no service").small());
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Blaze is a prototype. Browser rendering, media decode, and calling are not active yet.")
                        .small()
                        .weak(),
                );
            });
    }

    fn browser_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Browser");
        ui.label("Native shell with a strict URL boundary. Page rendering is awaiting the Servo adapter.");
        ui.add_space(10.0);

        let mut navigate = false;
        ui.horizontal(|ui| {
            if ui.button("←").on_hover_text("Back (engine pending)").clicked() {
                self.browser_status = "Back navigation requires the browser engine.".into();
            }
            if ui.button("→").on_hover_text("Forward (engine pending)").clicked() {
                self.browser_status = "Forward navigation requires the browser engine.".into();
            }
            if ui.button("↻").on_hover_text("Reload (engine pending)").clicked() {
                self.browser_status = "Reload requires the browser engine.".into();
            }
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.address)
                    .hint_text("Search or enter an address")
                    .desired_width(f32::INFINITY),
            );
            if input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                navigate = true;
            }
            if ui.button("Go").clicked() {
                navigate = true;
            }
            if ui.button("+").on_hover_text("New tab").clicked() {
                let id = self.tab_policy.open("about:blank");
                self.tabs.push(BrowserTab {
                    id,
                    title: "New tab".into(),
                    url: "about:blank".into(),
                });
                self.active_tab = self.tabs.len() - 1;
                self.address = "about:blank".into();
                self.tab_policy.enforce_live_limit();
            }
        });

        if navigate {
            self.navigate();
        }

        ui.horizontal_wrapped(|ui| {
            let mut selected = None;
            for (index, tab) in self.tabs.iter().enumerate() {
                let paused = self.tab_policy.entries().iter()
                    .find(|entry| entry.id() == tab.id)
                    .is_some_and(|entry| entry.activity == blaze_core::memory::TabActivity::Suspended);
                let label = if paused {
                    format!("◌ {}", tab.title)
                } else {
                    tab.title.clone()
                };
                if ui.selectable_label(self.active_tab == index, label).clicked() {
                    selected = Some(index);
                }
            }
            if let Some(index) = selected {
                self.active_tab = index;
                self.address = self.tabs[index].url.clone();
                self.tab_policy.select(self.tabs[index].id);
                self.tab_policy.enforce_live_limit();
            }
        });

        ui.add_space(16.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_height(330.0);
            ui.vertical_centered(|ui| {
                ui.add_space(62.0);
                ui.heading(egui::RichText::new("BLAZE").size(36.0).strong());
                ui.label("Fast by design. Private by default.");
                ui.add_space(18.0);
                ui.label(egui::RichText::new(&self.browser_status).color(egui::Color32::YELLOW));
                ui.add_space(12.0);
                ui.label("The URL is validated locally, but this build does not yet render remote pages.");
                ui.label("Next engine task: connect Servo's event loop, WebView, RenderingContext, and input events.");
            });
        });

        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Tab policy").strong());
            ui.label(format!(
                "{} live / {} open · {} MiB soft memory budget · maximum 8 live tabs",
                self.tab_policy.live_count(),
                self.tab_policy.entries().len(),
                self.tab_policy.budget().soft_limit_mib()
            ));
        });
    }

    fn navigate(&mut self) {
        match normalize_address(&self.address) {
            Ok(url) => {
                let normalized = url.to_string();
                self.address = normalized.clone();
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.url = normalized.clone();
                    tab.title = url.host_str().unwrap_or("Blank").to_owned();
                    self.tab_policy.set_url(tab.id, normalized.clone());
                }
                self.browser_status = format!(
                    "Address accepted: {normalized}. Engine status: not connected; no network page load occurred."
                );
            }
            Err(error) => {
                self.browser_status = format!("Navigation rejected: {error}");
            }
        }
    }

    fn code_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Code Studio");
        ui.label("A lightweight source buffer is ready. Language servers, terminal sandboxing, and WASM extensions are not connected.");
        ui.horizontal(|ui| {
            ui.label("File");
            ui.text_edit_singleline(&mut self.code_name);
            if ui.button("Save source").clicked() {
                let body = self.code_source.clone();
                self.save_named_file(&self.code_name.clone(), body.as_bytes());
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.code_source)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
    }

    fn documents_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Documents");
        ui.label("Markdown-first editor. DOCX import/export is not implemented in this prototype.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.document_title);
            if ui.button("Export Markdown").clicked() {
                let name = format!("{}.md", safe_stem(&self.document_title));
                let body = self.document_markdown.clone();
                self.save_named_file(&name, body.as_bytes());
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.document_markdown)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
    }

    fn sheets_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sheets");
        ui.label("CSV editor/export foundation. Formula calculation, XLSX interoperability, charts, and large-grid virtualization are pending.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.sheet_title);
            if ui.button("Export CSV").clicked() {
                let name = format!("{}.csv", safe_stem(&self.sheet_title));
                let body = self.sheet_csv.clone();
                self.save_named_file(&name, body.as_bytes());
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.sheet_csv)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
    }

    fn slides_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Presentations");
        ui.label("Slide-outline editor. PPTX layout, media, and export are pending.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.presentation_title);
            if ui.button("Export outline").clicked() {
                let name = format!("{}.md", safe_stem(&self.presentation_title));
                let body = self.slide_outline.clone();
                self.save_named_file(&name, body.as_bytes());
            }
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.slide_outline)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
    }

    fn notes_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Notes");
        ui.label("Write locally, then use Vault to save or restore this note as authenticated encrypted data.");
        ui.add(
            egui::TextEdit::multiline(&mut self.notes)
                .desired_width(f32::INFINITY)
                .desired_rows(24),
        );
        if ui.button("Open Vault").clicked() {
            self.section = Section::Vault;
        }
    }

    fn messenger_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Messenger");
        ui.label("Local message composer foundation. There is no account, relay, synchronization, or network transport yet.");
        ui.horizontal(|ui| {
            ui.label("Recipient");
            ui.text_edit_singleline(&mut self.message_to);
        });
        ui.add(
            egui::TextEdit::multiline(&mut self.message_body)
                .hint_text("Write a message draft...")
                .desired_width(f32::INFINITY)
                .desired_rows(10),
        );
        ui.label("Send is intentionally unavailable until identity, key verification, and a reviewed secure messaging protocol exist.");
    }

    fn calling_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Calling");
        ui.label("Calling UI foundation only. No camera/microphone permissions are requested in this build.");
        ui.add_space(8.0);
        ui.label(&self.call_status);
        ui.horizontal(|ui| {
            ui.add_enabled(false, egui::Button::new("Start audio call"));
            ui.add_enabled(false, egui::Button::new("Start video call"));
            if ui.button("Reset").clicked() {
                self.call_status = "Calling transport is not configured. No microphone or camera is accessed.".into();
            }
        });
    }

    fn vault_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Encrypted Vault");
        ui.label("Notes are encrypted before being written. Your passphrase is not saved by Blaze.");
        ui.label("Encryption: Argon2 key derivation + ChaCha20-Poly1305 authenticated encryption.");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Passphrase");
            ui.add(
                egui::TextEdit::singleline(&mut self.vault_password)
                    .password(true)
                    .desired_width(300.0)
                    .hint_text("Use a strong, unique passphrase"),
            );
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.vault_password.is_empty(), egui::Button::new("Save notes encrypted"))
                .clicked()
            {
                self.save_notes_to_vault();
            }
            if ui
                .add_enabled(!self.vault_password.is_empty(), egui::Button::new("Unlock notes"))
                .clicked()
            {
                self.load_notes_from_vault();
            }
        });
        ui.add_space(8.0);
        ui.label(&self.vault_status);
        ui.label("Losing the passphrase means the encrypted notes cannot be recovered. Keep a secure backup.");
        ui.label("This prototype does not yet integrate an OS keychain or an encrypted browser profile database.");
    }

    fn save_notes_to_vault(&mut self) {
        let Some(path) = vault_file_path() else {
            self.vault_status = "Could not resolve an application data directory.".into();
            return;
        };
        match write_encrypted(&path, self.vault_password.as_bytes(), self.notes.as_bytes()) {
            Ok(()) => {
                self.vault_status = format!("Encrypted notes saved to {}.", path.display());
                self.toast = "Encrypted notes saved.".into();
            }
            Err(error) => {
                self.vault_status = format!("Vault save failed: {error}");
            }
        }
    }

    fn load_notes_from_vault(&mut self) {
        let Some(path) = vault_file_path() else {
            self.vault_status = "Could not resolve an application data directory.".into();
            return;
        };
        match read_encrypted(&path, self.vault_password.as_bytes()) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(contents) => {
                    self.notes = contents;
                    self.vault_status = "Notes authenticated and unlocked.".into();
                    self.toast = "Vault unlocked.".into();
                }
                Err(_) => {
                    self.vault_status = "The decrypted file is not valid UTF-8 text.".into();
                }
            },
            Err(error) => {
                self.vault_status = format!("Vault unlock failed: {error}");
            }
        }
    }

    fn save_named_file(&mut self, name: &str, bytes: &[u8]) {
        let path = safe_output_path(name);
        match path {
            Some(path) => match fs::write(&path, bytes) {
                Ok(()) => self.toast = format!("Saved {}", path.display()),
                Err(error) => self.toast = format!("Save failed: {error}"),
            },
            None => self.toast = "Invalid file name.".into(),
        }
    }
}

fn safe_stem(input: &str) -> String {
    let filtered: String = input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-' || *character == '_')
        .take(60)
        .collect();
    if filtered.is_empty() {
        "blaze-export".into()
    } else {
        filtered
    }
}

fn safe_output_path(name: &str) -> Option<PathBuf> {
    let file_name = Path::new(name).file_name()?.to_str()?;
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return None;
    }
    std::env::current_dir().ok().map(|directory| directory.join(file_name))
}

fn vault_file_path() -> Option<PathBuf> {
    let project = ProjectDirs::from("dev", "Blaze", "Blaze")?;
    Some(project.data_dir().join("vault").join("notes.blz"))
}
