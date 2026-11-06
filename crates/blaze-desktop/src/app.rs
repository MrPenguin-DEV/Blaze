use blaze_browser::normalize_address;
use blaze_core::{
    memory::MemoryBudget,
    tabs::TabManager,
    TabId,
};
use blaze_profile::{BrowserProfile, PermissionDecision, PermissionKind, SessionTab};
use blaze_vault::{read_encrypted, write_encrypted};
use directories::ProjectDirs;
use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    Privacy,
}

impl Section {
    const ALL: [(Self, &'static str); 10] = [
        (Self::Browser, "Browser"),
        (Self::Code, "Code Studio"),
        (Self::Documents, "Documents"),
        (Self::Sheets, "Sheets"),
        (Self::Slides, "Presentations"),
        (Self::Notes, "Notes"),
        (Self::Messenger, "Messenger"),
        (Self::Calling, "Calling"),
        (Self::Vault, "Vault"),
        (Self::Privacy, "Privacy & Permissions"),
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
    wasm_path: String,
    wasm_status: String,
    document_title: String,
    document_markdown: String,
    sheet_title: String,
    sheet_csv: String,
    sheet_formula: String,
    sheet_formula_result: String,
    presentation_title: String,
    slide_outline: String,
    office_import_path: String,
    office_export_path: String,
    notes: String,
    message_to: String,
    message_body: String,
    message_log: String,
    signal_input: String,
    signal_output: String,
    trusted_peer_fingerprint: String,
    remote_fingerprint: String,
    local_fingerprint: String,
    identity_verified: bool,
    comms_status: String,
    comms_runtime: tokio::runtime::Runtime,
    peer_session: Option<blaze_comms::PeerSession>,
    call_status: String,
    vault_password: String,
    vault_status: String,
    profile: BrowserProfile,
    profile_passphrase: String,
    profile_status: String,
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
            wasm_path: "extension.wasm".into(),
            wasm_status: "Wasm runner: choose a compiled module with a no-argument run export.".into(),
            document_title: "Untitled document".into(),
            document_markdown: "# Untitled document\n\nStart writing here.\n".into(),
            sheet_title: "Untitled spreadsheet".into(),
            sheet_csv: "Name,Value\nBlaze,1\n".into(),
            sheet_formula: "=SUM(B2:B2)".into(),
            sheet_formula_result: "Enter a formula to calculate against the CSV grid.".into(),
            presentation_title: "Untitled presentation".into(),
            office_import_path: "document.docx".into(),
            office_export_path: "document.docx".into(),
            slide_outline: "# Slide 1 — Welcome\n\nBlaze is a Rust-first workspace.\n\n---\n\n# Slide 2 — Roadmap\n\nAdd your next slide here.\n".into(),
            notes: String::new(),
            message_to: String::new(),
            message_body: String::new(),
            message_log: String::new(),
            signal_input: String::new(),
            signal_output: String::new(),
            trusted_peer_fingerprint: String::new(),
            remote_fingerprint: String::new(),
            local_fingerprint: String::new(),
            identity_verified: false,
            comms_status: "No peer connection. Create a connection, exchange signaling messages with a trusted contact, then verify the remote fingerprint.".into(),
            comms_runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Could not create Blaze communications runtime"),
            peer_session: None,
            call_status: "Calling transport is not configured. No microphone or camera is accessed.".into(),
            vault_password: String::new(),
            vault_status: "Vault is locked. Enter a strong passphrase to load or save encrypted notes.".into(),
            profile: BrowserProfile::default(),
            profile_passphrase: String::new(),
            profile_status: "Profile is in memory only until you save it encrypted.".into(),
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
                Section::Privacy => self.privacy_panel(ui),
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
                ui.label(egui::RichText::new("Engine: Servo host is optional").small());
                ui.label(egui::RichText::new("Vault: password locked").small());
                ui.label(egui::RichText::new("Profile: encrypted local").small());
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
        ui.label("The main shell can launch the standalone Servo renderer when built with the optional servo-host feature.");
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
            #[cfg(feature = "servo-host")]
            if ui.button("Open in Servo").clicked() {
                match launch_servo(&self.address, &self.profile) {
                    Ok(()) => self.browser_status = "Opened the current address in the separate Servo browser window.".into(),
                    Err(error) => self.browser_status = format!("Could not launch Servo: {error}"),
                }
            }
            #[cfg(not(feature = "servo-host"))]
            if ui.button("Servo host instructions").clicked() {
                self.browser_status = "Build both binaries with: cargo build -p blaze-desktop --features servo-host --bins".into();
            }
            if ui.button("☆").on_hover_text("Bookmark current page").clicked() {
                let title = self.tabs.get(self.active_tab).map(|tab| tab.title.clone()).unwrap_or_else(|| "Bookmark".into());
                self.profile.add_bookmark(title, self.address.clone());
                self.toast = "Bookmark added to this profile.".into();
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
                ui.label("Remote pages render in the separate blaze-servo process when built with --features servo-host --bins.");
                ui.label("The windows use separate rendering contexts; a direct in-process wgpu/Servo texture bridge is still missing.");
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
                let title = url.host_str().unwrap_or("Blank").to_owned();
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.url = normalized.clone();
                    tab.title = title.clone();
                    self.tab_policy.set_url(tab.id, normalized.clone());
                }
                self.profile.record_visit(title, normalized.clone(), unix_now(), 500);
                self.browser_status = format!(
                    "Address accepted: {normalized}. Use the separate blaze-servo host to render live pages."
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
                .desired_rows(18),
        );

        ui.separator();
        ui.label(egui::RichText::new("WASM compute sandbox").strong());
        ui.label("Runs a compiled WebAssembly binary with no host imports. It cannot access files, network, browser state, clipboard, or devices.");
        ui.horizontal(|ui| {
            ui.label("Module");
            ui.text_edit_singleline(&mut self.wasm_path);
            if ui.button("Run .wasm").clicked() {
                self.run_wasm_module();
            }
        });
        ui.label("Entry point: run() with no arguments and no return value. Fuel: 5,000,000. Guest memory: 32 MiB.");
        ui.label(&self.wasm_status);
    }

    fn run_wasm_module(&mut self) {
        let path = PathBuf::from(self.wasm_path.trim());
        match fs::read(&path) {
            Ok(bytes) => match blaze_wasm::run_compute_module(
                &bytes,
                "run",
                blaze_wasm::SandboxConfig::default(),
            ) {
                Ok(report) => {
                    self.wasm_status = format!(
                        "Execution completed. Fuel used: {} / {} ({} remaining).",
                        report.fuel_consumed,
                        report.fuel_limit,
                        report.fuel_remaining
                    );
                    self.toast = format!("WASM module {} completed.", path.display());
                }
                Err(error) => {
                    self.wasm_status = format!("WASM execution rejected or trapped: {error}");
                }
            },
            Err(error) => {
                self.wasm_status = format!("Could not read {}: {error}", path.display());
            }
        }
    }

    fn documents_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Documents");
        ui.label("Markdown editor with DOCX import/export through the Office bridge.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.document_title);
        });
        ui.horizontal(|ui| {
            ui.label("Import path");
            ui.text_edit_singleline(&mut self.office_import_path);
            if ui.button("Import Office").clicked() {
                let path = self.office_import_path.clone();
                match blaze_office::import_office(&path) {
                    Ok(imported) => {
                        self.document_markdown = imported.markdown;
                        self.toast = format!("Imported {:?}; {} parser warnings.", imported.format, imported.damage_count);
                    }
                    Err(error) => self.toast = format!("Office import failed: {error}"),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Export path");
            ui.text_edit_singleline(&mut self.office_export_path);
            if ui.button("Export DOCX").clicked() {
                let path = self.office_export_path.clone();
                match blaze_office::export_markdown(
                    &self.document_markdown,
                    blaze_office::OfficeOutputFormat::Docx,
                    &path,
                ) {
                    Ok(()) => self.toast = format!("Exported DOCX to {path}"),
                    Err(error) => self.toast = format!("DOCX export failed: {error}"),
                }
            }
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
        ui.label("CSV editor, XLSX import/export and Excel-style formula calculation. Charts and large-grid virtualization remain pending.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.sheet_title);
        });
        ui.horizontal(|ui| {
            ui.label("Import path");
            ui.text_edit_singleline(&mut self.office_import_path);
            if ui.button("Import XLSX / Office").clicked() {
                let path = self.office_import_path.clone();
                match blaze_office::import_spreadsheet_as_csv(&path) {
                    Ok(imported) => {
                        self.sheet_csv = imported.csv;
                        self.toast = format!("Imported {:?} as CSV; {} parser warnings.", imported.format, imported.damage_count);
                    }
                    Err(error) => self.toast = format!("Spreadsheet import failed: {error}"),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Export path");
            ui.text_edit_singleline(&mut self.office_export_path);
            if ui.button("Export XLSX").clicked() {
                let path = self.office_export_path.clone();
                match blaze_office::export_csv(&self.sheet_csv, &path) {
                    Ok(()) => self.toast = format!("Exported XLSX to {path}"),
                    Err(error) => self.toast = format!("XLSX export failed: {error}"),
                }
            }
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
                .desired_rows(18),
        );
        ui.separator();
        ui.label(egui::RichText::new("Formula bar").strong());
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.sheet_formula);
            if ui.button("Evaluate").clicked() {
                match blaze_office::evaluate_csv_formula(&self.sheet_csv, &self.sheet_formula) {
                    Ok(value) => self.sheet_formula_result = value,
                    Err(error) => self.sheet_formula_result = format!("Formula error: {error}"),
                }
            }
        });
        ui.label(format!("Result: {}", self.sheet_formula_result));
        ui.label("Formula references use A1 notation. The evaluator supports a wide Excel-style function set and caps materialized ranges at 10,000 cells.");
    }

    fn slides_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Presentations");
        ui.label("Slide-outline editor with basic PPTX import/export. Full slide layout, themes, and media editing are still pending.");
        ui.horizontal(|ui| {
            ui.label("Title");
            ui.text_edit_singleline(&mut self.presentation_title);
        });
        ui.horizontal(|ui| {
            ui.label("Import path");
            ui.text_edit_singleline(&mut self.office_import_path);
            if ui.button("Import PPTX / Office").clicked() {
                let path = self.office_import_path.clone();
                match blaze_office::import_office(&path) {
                    Ok(imported) => {
                        self.slide_outline = imported.markdown;
                        self.toast = format!("Imported {:?}; {} parser warnings.", imported.format, imported.damage_count);
                    }
                    Err(error) => self.toast = format!("Presentation import failed: {error}"),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Export path");
            ui.text_edit_singleline(&mut self.office_export_path);
            if ui.button("Export PPTX").clicked() {
                let path = self.office_export_path.clone();
                match blaze_office::export_markdown(
                    &self.slide_outline,
                    blaze_office::OfficeOutputFormat::Pptx,
                    &path,
                ) {
                    Ok(()) => self.toast = format!("Exported PPTX to {path}"),
                    Err(error) => self.toast = format!("PPTX export failed: {error}"),
                }
            }
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
        self.poll_peer_queues();
        ui.label("Peer-to-peer WebRTC text channel with manual copy/paste signaling. No central signaling server is configured.");
        ui.label(egui::RichText::new(blaze_comms::IDENTITY_WARNING).color(egui::Color32::YELLOW));
        ui.label("Your SHA-256 DTLS fingerprint (share through a separate trusted channel):");
        ui.label(if self.local_fingerprint.is_empty() { "Not generated yet.".to_owned() } else { self.local_fingerprint.clone() });
        ui.label("Expected peer SHA-256 fingerprint (confirm it independently with that person):");
        ui.text_edit_singleline(&mut self.trusted_peer_fingerprint);
        ui.label(if self.identity_verified {
            "Peer fingerprint matched the value you entered.".to_owned()
        } else {
            "Peer identity is not verified. Sending is disabled.".to_owned()
        });
        if !self.remote_fingerprint.is_empty() {
            ui.label(format!("Fingerprint in the pasted remote SDP: {}", self.remote_fingerprint));
        }
        ui.horizontal(|ui| {
            if self.peer_session.is_none() {
                if ui.button("Create peer connection").clicked() {
                    match self.comms_runtime.block_on(blaze_comms::PeerSession::new(
                        blaze_comms::PeerConfig::default(),
                    )) {
                        Ok(session) => {
                            self.peer_session = Some(session);
                            self.identity_verified = false;
                            self.local_fingerprint.clear();
                            self.remote_fingerprint.clear();
                            self.comms_status = "Peer connection created. Create an offer, then exchange signaling JSON and verify the SHA-256 fingerprints out of band.".into();
                        }
                        Err(error) => self.comms_status = format!("Could not create peer connection: {error}"),
                    }
                }
            } else {
                if ui.button("Close connection").clicked() {
                    if let Some(session) = self.peer_session.take() {
                        match self.comms_runtime.block_on(session.close()) {
                            Ok(()) => self.comms_status = "Peer connection closed.".into(),
                            Err(error) => self.comms_status = format!("Close reported: {error}"),
                        }
                    }
                    self.identity_verified = false;
                    self.local_fingerprint.clear();
                    self.remote_fingerprint.clear();
                }
                if ui.button("Create offer").clicked() {
                    let result = self.peer_session.as_ref().map(|session| {
                        self.comms_runtime.block_on(session.create_offer())
                    });
                    match result {
                        Some(Ok(description)) => {
                            self.local_fingerprint = blaze_comms::sha256_dtls_fingerprint(&description.sdp)
                                .unwrap_or_default();
                            self.append_signaling_output(blaze_comms::SignalingMessage::Description(description));
                            self.comms_status = "Offer created. Send the JSON to your contact over a trusted channel, and share your local fingerprint for verification.".into();
                        }
                        Some(Err(error)) => self.comms_status = format!("Offer creation failed: {error}"),
                        None => {}
                    }
                }
            }
        });

        ui.label("Recipient / contact label (local only)");
        ui.text_edit_singleline(&mut self.message_to);
        ui.label("Remote signaling messages — one JSON object per line (offer, answer, or ICE candidate)");
        ui.add(
            egui::TextEdit::multiline(&mut self.signal_input)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(5),
        );
        if ui.add_enabled(self.peer_session.is_some() && !self.signal_input.trim().is_empty(), egui::Button::new("Apply remote signaling")).clicked() {
            self.apply_remote_signaling();
        }

        ui.label("Local signaling output — send each JSON line to the other peer through a separate trusted channel");
        ui.add(
            egui::TextEdit::multiline(&mut self.signal_output)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(5),
        );
        ui.horizontal(|ui| {
            if ui.button("Clear signaling output").clicked() {
                self.signal_output.clear();
            }
            if ui.button("Poll incoming messages").clicked() {
                self.poll_peer_queues();
            }
        });
        ui.label(&self.comms_status);

        ui.separator();
        ui.label(egui::RichText::new("Conversation").strong());
        ui.add(
            egui::TextEdit::multiline(&mut self.message_log)
                .desired_width(f32::INFINITY)
                .desired_rows(7)
                .interactive(false),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.message_body)
                    .hint_text("Write a message...")
                    .desired_width(f32::INFINITY),
            );
            if ui.add_enabled(self.peer_session.is_some() && self.identity_verified && !self.message_body.trim().is_empty(), egui::Button::new("Send")).clicked() {
                let message = std::mem::take(&mut self.message_body);
                let result = self.peer_session.as_ref().map(|session| {
                    self.comms_runtime.block_on(async {
                        tokio::time::timeout(Duration::from_secs(8), async {
                            session.wait_until_writable().await?;
                            session.send_text(&message).await
                        })
                        .await
                        .map_err(|_| "timed out waiting for the peer data channel to open".to_owned())?
                        .map_err(|error| error.to_string())
                    })
                });
                match result {
                    Some(Ok(())) => {
                        self.message_log.push_str(&format!("Me: {message}\n"));
                        self.comms_status = "Message sent over the verified WebRTC data channel.".into();
                    }
                    Some(Err(error)) => {
                        self.message_body = message;
                        self.comms_status = format!("Message was not sent: {error}");
                    }
                    None => {}
                }
            }
        });
        ui.label("Connection setup uses host candidates only by default. Peers on different networks may need an explicitly configured TURN server. Never send signaling JSON through an untrusted channel.");
    }

    fn append_signaling_output(&mut self, message: blaze_comms::SignalingMessage) {
        if let blaze_comms::SignalingMessage::Description(description) = &message {
            self.local_fingerprint =
                blaze_comms::sha256_dtls_fingerprint(&description.sdp).unwrap_or_default();
        }
        match serde_json::to_string(&message) {
            Ok(json) => {
                self.signal_output.push_str(&json);
                self.signal_output.push('\n');
            }
            Err(error) => self.comms_status = format!("Could not serialize signaling message: {error}"),
        }
    }

    fn apply_remote_signaling(&mut self) {
        let lines: Vec<String> = self
            .signal_input
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect();

        let mut messages = Vec::with_capacity(lines.len());
        for line in &lines {
            match serde_json::from_str::<blaze_comms::SignalingMessage>(line) {
                Ok(message) => messages.push(message),
                Err(error) => {
                    self.comms_status = format!("Invalid signaling JSON: {error}");
                    return;
                }
            }
        }

        let remote_descriptions = messages
            .iter()
            .filter_map(|message| match message {
                blaze_comms::SignalingMessage::Description(description) => Some(description),
                blaze_comms::SignalingMessage::Candidate(_) => None,
            })
            .collect::<Vec<_>>();

        if !remote_descriptions.is_empty() {
            for description in remote_descriptions {
                let Some(fingerprint) = blaze_comms::sha256_dtls_fingerprint(&description.sdp) else {
                    self.identity_verified = false;
                    self.comms_status = "Remote SDP has no single unambiguous SHA-256 DTLS fingerprint. Signaling was not applied.".into();
                    return;
                };
                self.remote_fingerprint = fingerprint;

                if !blaze_comms::verify_sha256_dtls_fingerprint(
                    &description.sdp,
                    &self.trusted_peer_fingerprint,
                ) {
                    self.identity_verified = false;
                    self.comms_status = "Fingerprint mismatch or missing expected value. Verify the fingerprint independently, enter it above, and apply signaling again.".into();
                    return;
                }
            }
            self.identity_verified = true;
        } else if !self.identity_verified {
            self.comms_status = "A peer SDP must be fingerprint-verified before ICE candidates or messages can be accepted.".into();
            return;
        }

        self.signal_input.clear();
        for message in messages {
            let result = self.peer_session.as_ref().map(|session| {
                self.comms_runtime.block_on(session.handle_signaling(message))
            });
            match result {
                Some(Ok(Some(response))) => {
                    self.append_signaling_output(response);
                    self.comms_status = "Remote offer applied and fingerprint verified; local answer added to signaling output.".into();
                }
                Some(Ok(None)) => {
                    self.comms_status = "Remote signaling message applied.".into();
                }
                Some(Err(error)) => {
                    self.comms_status = format!("Could not apply remote signaling: {error}");
                    return;
                }
                None => break,
            }
        }
    }

    fn poll_peer_queues(&mut self) {
        let (outbound, incoming) = match self.peer_session.as_mut() {
            Some(session) => (
                session.drain_outbound_signaling(),
                session.drain_incoming_text(),
            ),
            None => return,
        };

        for message in outbound {
            self.append_signaling_output(message);
        }
        for message in incoming {
            self.message_log.push_str(&format!("Peer: {message}\n"));
        }
    }

    fn calling_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Calling");
        ui.label("The WebRTC module currently supports data-channel messaging only. Audio/video calling still needs device capture, media tracks, authenticated signaling, and permission integration.");
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


    fn privacy_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Privacy & Permissions");
        ui.label("Site permissions default to Ask. Only an explicit Allow decision is considered granted.");
        ui.add_space(8.0);

        ui.checkbox(&mut self.profile.settings.do_not_track, "Send Do Not Track preference");
        ui.checkbox(&mut self.profile.settings.global_privacy_control, "Send Global Privacy Control signal");
        ui.checkbox(&mut self.profile.settings.block_third_party_cookies, "Block third-party cookies (policy preference)");
        ui.checkbox(&mut self.profile.settings.telemetry_enabled, "Enable optional Blaze telemetry (off by default)");
        ui.checkbox(&mut self.profile.settings.restore_last_session, "Restore last session metadata");

        ui.separator();
        ui.label(egui::RichText::new("Permissions for current address").strong());
        let current_url = self.address.clone();
        ui.label(&current_url);
        let permission_kinds = [
            PermissionKind::Camera,
            PermissionKind::Microphone,
            PermissionKind::Geolocation,
            PermissionKind::Notifications,
            PermissionKind::ClipboardRead,
            PermissionKind::ClipboardWrite,
            PermissionKind::Downloads,
            PermissionKind::Popups,
        ];
        for kind in permission_kinds {
            let decision = self.profile
                .permission_for(&current_url, kind)
                .unwrap_or(PermissionDecision::Ask);
            ui.horizontal(|ui| {
                ui.label(format!("{kind:?}: {decision:?}"));
                if ui.small_button("Allow").clicked() {
                    match self.profile.set_permission(&current_url, kind, PermissionDecision::Allow) {
                        Ok(()) => self.profile_status = format!("{kind:?} allowed for this origin."),
                        Err(error) => self.profile_status = format!("Permission change rejected: {error}"),
                    }
                }
                if ui.small_button("Deny").clicked() {
                    match self.profile.set_permission(&current_url, kind, PermissionDecision::Deny) {
                        Ok(()) => self.profile_status = format!("{kind:?} denied for this origin."),
                        Err(error) => self.profile_status = format!("Permission change rejected: {error}"),
                    }
                }
                if ui.small_button("Ask").clicked() {
                    match self.profile.set_permission(&current_url, kind, PermissionDecision::Ask) {
                        Ok(()) => self.profile_status = format!("{kind:?} reset to Ask."),
                        Err(error) => self.profile_status = format!("Permission change rejected: {error}"),
                    }
                }
            });
        }

        ui.separator();
        ui.label(egui::RichText::new("Bookmarks").strong());
        let bookmarks = self.profile.bookmarks.clone();
        let mut open_bookmark: Option<String> = None;
        let mut remove_bookmark: Option<String> = None;
        for bookmark in bookmarks {
            ui.horizontal(|ui| {
                ui.label(&bookmark.title);
                ui.small(&bookmark.url);
                if ui.small_button("Open").clicked() {
                    open_bookmark = Some(bookmark.url.clone());
                }
                if ui.small_button("Remove").clicked() {
                    remove_bookmark = Some(bookmark.url.clone());
                }
            });
        }
        if let Some(url) = open_bookmark {
            self.address = url;
            self.section = Section::Browser;
            self.navigate();
        }
        if let Some(url) = remove_bookmark {
            self.profile.remove_bookmark(&url);
        }

        ui.separator();
        ui.label(egui::RichText::new("Encrypted browser profile").strong());
        ui.label("Bookmarks, capped visit history, site permissions, and privacy settings are encrypted together.");
        ui.horizontal(|ui| {
            ui.label("Profile passphrase");
            ui.add(
                egui::TextEdit::singleline(&mut self.profile_passphrase)
                    .password(true)
                    .desired_width(300.0)
                    .hint_text("Strong unique passphrase"),
            );
        });
        ui.horizontal(|ui| {
            if ui.add_enabled(!self.profile_passphrase.is_empty(), egui::Button::new("Save encrypted profile")).clicked() {
                self.save_profile();
            }
            if ui.add_enabled(!self.profile_passphrase.is_empty(), egui::Button::new("Unlock profile")).clicked() {
                self.load_profile();
            }
        });
        ui.label(&self.profile_status);
        ui.label("When launched from this shell, the current origin's saved permission snapshot is passed to Servo. Only explicit Allow decisions for the matching top-level origin are granted; unsupported permission features remain denied.");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Recent history").strong());
            if ui.button("Clear history").clicked() {
                self.profile.clear_history();
                self.profile_status = "Browsing history cleared from the in-memory profile. Save the encrypted profile to persist the change.".into();
            }
        });
        let recent_history = self.profile.history.iter().take(12).cloned().collect::<Vec<_>>();
        for entry in recent_history {
            ui.horizontal(|ui| {
                ui.label(&entry.title);
                ui.small(&entry.url);
            });
        }
        ui.label(format!(
            "{} history entries retained in memory.",
            self.profile.history.len()
        ));
    }

    fn save_profile(&mut self) {
        let session: Vec<SessionTab> = self
            .tabs
            .iter()
            .map(|tab| {
                let pinned = self
                    .tab_policy
                    .entries()
                    .iter()
                    .find(|entry| entry.id() == tab.id)
                    .is_some_and(|entry| entry.snapshot.pinned);
                SessionTab {
                    title: tab.title.clone(),
                    url: tab.url.clone(),
                    pinned,
                }
            })
            .collect();

        if self.profile.settings.restore_last_session {
            self.profile.set_last_session(session);
        } else {
            self.profile.set_last_session(Vec::new());
        }

        let Some(path) = profile_file_path() else {
            self.profile_status = "Could not resolve the profile data directory.".into();
            return;
        };
        match self.profile.save_encrypted(&path, self.profile_passphrase.as_bytes()) {
            Ok(()) => {
                self.profile_status = format!("Encrypted profile saved to {}.", path.display());
                self.toast = "Encrypted profile saved.".into();
            }
            Err(error) => self.profile_status = format!("Profile save failed: {error}"),
        }
    }

    fn load_profile(&mut self) {
        let Some(path) = profile_file_path() else {
            self.profile_status = "Could not resolve the profile data directory.".into();
            return;
        };
        match BrowserProfile::load_encrypted(&path, self.profile_passphrase.as_bytes()) {
            Ok(profile) => {
                self.profile = profile;
                if self.profile.settings.restore_last_session {
                    let saved_tabs = self.profile.last_session.clone();
                    if !saved_tabs.is_empty() {
                        self.tab_policy = TabManager::new(MemoryBudget::new(1024), 8);
                        self.tabs.clear();

                        for saved in saved_tabs {
                            let Ok(url) = normalize_address(&saved.url) else {
                                continue;
                            };
                            if !matches!(url.scheme(), "http" | "https") {
                                continue;
                            }
                            let normalized = url.to_string();
                            let id = self.tab_policy.open(normalized.clone());
                            let _ = self.tab_policy.set_pinned(id, saved.pinned);
                            self.tabs.push(BrowserTab {
                                id,
                                title: saved.title,
                                url: normalized,
                            });
                        }

                        if self.tabs.is_empty() {
                            let id = self.tab_policy.open("about:blank");
                            self.tabs.push(BrowserTab {
                                id,
                                title: "New tab".into(),
                                url: "about:blank".into(),
                            });
                        }

                        self.active_tab = self.tabs.len() - 1;
                        let selected = self.tabs[self.active_tab].clone();
                        self.address = selected.url;
                        self.tab_policy.select(selected.id);
                        let _ = self.tab_policy.enforce_live_limit();
                    }
                }
                self.profile_status = format!(
                    "Profile authenticated and unlocked. Restored {} tab entries; pages must be opened in the Servo host.",
                    self.tabs.len()
                );
                self.toast = "Browser profile unlocked.".into();
            }
            Err(error) => self.profile_status = format!("Profile unlock failed: {error}"),
        }
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

#[cfg(feature = "servo-host")]
fn launch_servo(address: &str, profile: &BrowserProfile) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let servo_exe = current_exe.with_file_name(format!("blaze-servo{}", std::env::consts::EXE_SUFFIX));
    // Only copy permission preferences into the child process. Never pass the
    // profile passphrase, history, bookmarks, or encrypted profile contents.
    let permission_snapshot = serde_json::to_string(&profile.permissions)
        .map_err(|error| format!("Could not serialize site permissions: {error}"))?;
    std::process::Command::new(&servo_exe)
        .arg(address)
        .env("BLAZE_SITE_PERMISSIONS", permission_snapshot)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("{} ({error}). Build with --features servo-host --bins.", servo_exe.display()))
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

fn profile_file_path() -> Option<PathBuf> {
    let project = ProjectDirs::from("dev", "Blaze", "Blaze")?;
    Some(project.data_dir().join("profile").join("profile.blz"))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
