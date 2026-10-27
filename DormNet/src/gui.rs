//! Compact Neon Dark credential UI for KIOIS Net (egui/eframe).
//!
//! Preserves existing handlers: save+login, save-only, clear form,
//! delete stored credentials (Windows Credential Manager).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use dormnet::auth::{AuthError, AuthProvider, MikroTikHotspotAuth};
use dormnet::creds::{self, Credentials};
use dormnet::network::{Connectivity, NetworkMonitor};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Margin, RichText, Sense, Stroke, TextEdit, TextureHandle, TextureOptions, Ui, Vec2,
};

pub const APP_DISPLAY_NAME: &str = "KIOIS เน็ตหอ";
pub const APP_DISPLAY_NAME_EN: &str = "KIOIS Net";
pub const APP_CREATOR: &str = "KIOIS";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Neon Dark palette (spec)
const BG: Color32 = Color32::from_rgb(0x07, 0x11, 0x1F);
const PANEL: Color32 = Color32::from_rgb(0x0D, 0x1B, 0x2D);
const FIELD: Color32 = Color32::from_rgb(0x0A, 0x16, 0x26);
const ACCENT: Color32 = Color32::from_rgb(0x00, 0xE5, 0xD4);
const ACCENT_DIM: Color32 = Color32::from_rgb(0x00, 0xB8, 0xAA);
const BLUE: Color32 = Color32::from_rgb(0x25, 0x83, 0xFF);
const TEXT: Color32 = Color32::from_rgb(0xF3, 0xF7, 0xFF);
const MUTED: Color32 = Color32::from_rgb(0x91, 0xA4, 0xBD);
const BORDER: Color32 = Color32::from_rgb(0x20, 0x3A, 0x58);
const OK: Color32 = Color32::from_rgb(0x2E, 0xD3, 0xA5);
const DANGER: Color32 = Color32::from_rgb(0xE8, 0x5D, 0x6C);
const DANGER_SOFT: Color32 = Color32::from_rgb(0x3A, 0x18, 0x22);
const REQUIRED: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x7A);

const SPACE: f32 = 12.0;
const RADIUS: u8 = 8;
const CTRL_H: f32 = 40.0;
const SIDE: f32 = 24.0;
const PAD_Y: i8 = 20;
const COL_GAP: f32 = 10.0;

enum JobResult {
    Saved,
    DeletedStored,
    LoginOk,
    Err(String),
    NetStatus(Option<bool>, String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfirmKind {
    None,
    DeleteStored,
}

struct App {
    username: String,
    password: String,
    show_password: bool,
    status: String,
    status_ok: bool,
    online: Option<bool>,
    online_label: String,
    busy: bool,
    confirm: ConfirmKind,
    has_stored: bool,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    logo: Option<TextureHandle>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let (username, password, has_stored) = match creds::load() {
            Ok(c) => (c.username, c.password, true),
            Err(_) => (String::new(), String::new(), false),
        };
        let app = Self {
            username,
            password,
            show_password: false,
            status: String::new(),
            status_ok: true,
            online: None,
            online_label: "กำลังตรวจสอบการเชื่อมต่อ...".into(),
            busy: false,
            confirm: ConfirmKind::None,
            has_stored,
            tx,
            rx,
            logo: load_texture(ctx, "logo", include_bytes!("../assets/window-icon.png")),
        };
        app.refresh_net();
        app
    }

    fn set_status(&mut self, ok: bool, msg: impl Into<String>) {
        self.status_ok = ok;
        self.status = msg.into();
    }

    fn pump(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                JobResult::NetStatus(online, label) => {
                    self.online = online;
                    self.online_label = label;
                }
                other => {
                    self.busy = false;
                    match other {
                        JobResult::Saved => {
                            self.has_stored = true;
                            self.set_status(true, "บันทึกรหัสเรียบร้อยแล้ว");
                        }
                        JobResult::DeletedStored => {
                            self.has_stored = false;
                            self.username.clear();
                            self.password.clear();
                            self.set_status(true, "ลบรหัสที่เก็บไว้แล้วเรียบร้อย");
                        }
                        JobResult::LoginOk => {
                            self.has_stored = true;
                            self.set_status(true, "บันทึกแล้ว และเชื่อมต่อเน็ตสำเร็จ");
                            self.refresh_net();
                        }
                        JobResult::Err(e) => self.set_status(false, e),
                        JobResult::NetStatus(_, _) => {}
                    }
                }
            }
        }
    }

    fn refresh_net(&self) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let Ok(mon) = NetworkMonitor::new() else {
                let _ = tx.send(JobResult::NetStatus(
                    None,
                    "ตรวจสอบเน็ตไม่สำเร็จ".into(),
                ));
                return;
            };
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                let _ = tx.send(JobResult::NetStatus(
                    None,
                    "ตรวจสอบเน็ตไม่สำเร็จ".into(),
                ));
                return;
            };
            match rt.block_on(mon.check_connectivity()) {
                Connectivity::Online => {
                    let _ = tx.send(JobResult::NetStatus(Some(true), "เชื่อมต่อแล้ว".into()));
                }
                Connectivity::Captive => {
                    let _ = tx.send(JobResult::NetStatus(
                        Some(false),
                        "ยังไม่ได้ล็อกอินเน็ตหอ".into(),
                    ));
                }
                Connectivity::Offline => {
                    let _ = tx.send(JobResult::NetStatus(Some(false), "ยังไม่มีเน็ต".into()));
                }
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump();

        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(BG)
                    .inner_margin(Margin::symmetric(0, PAD_Y)),
            )
            .show(ctx, |ui| {
                // Equal gutters: fixed SIDE on both sides + locked content column width.
                let avail = ui.available_width();
                let w = (avail - SIDE * 2.0).floor().max(200.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(SIDE);
                    ui.allocate_ui_with_layout(
                        Vec2::new(w, ui.available_height()),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_min_size(Vec2::new(w, 0.0));
                            ui.set_max_width(w);
                            self.draw_header(ui, w);
                            ui.add_space(SPACE);
                            self.draw_form(ui, w);
                            ui.add_space(SPACE + 4.0);
                            self.draw_actions(ui, w);
                            ui.add_space(SPACE);
                            self.draw_feedback(ui, w);
                            ui.add_space(SPACE);
                            self.draw_footer(ui);
                        },
                    );
                    ui.add_space(SIDE);
                });

                if self.confirm == ConfirmKind::DeleteStored {
                    self.draw_confirm_delete(ui.ctx().clone());
                }
            });

        if self.busy || self.online.is_none() {
            ctx.request_repaint();
        }
    }
}

impl App {
    fn draw_header(&mut self, ui: &mut Ui, w: f32) {
        ui.allocate_ui_with_layout(
            Vec2::new(w, 44.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                if let Some(logo) = &self.logo {
                    ui.add(
                        egui::Image::new(logo)
                            .fit_to_exact_size(Vec2::splat(40.0))
                            .corner_radius(CornerRadius::same(8)),
                    );
                } else {
                    paint_mark(ui, 40.0);
                }
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(APP_DISPLAY_NAME)
                            .font(FontId::proportional(20.0))
                            .color(TEXT)
                            .strong(),
                    );
                    ui.label(
                        RichText::new("ช่วยจำรหัสเน็ตหอ แล้วต่อให้อัตโนมัติ")
                            .size(12.5)
                            .color(MUTED),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.draw_status_chip(ui);
                    ui.add_space(6.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("เช็กเน็ต").size(11.5).color(BLUE))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                                .frame(false),
                        )
                        .clicked()
                        && !self.busy
                    {
                        self.online = None;
                        self.online_label = "กำลังตรวจสอบการเชื่อมต่อ...".into();
                        self.refresh_net();
                    }
                });
            },
        );
    }

    fn draw_status_chip(&self, ui: &mut Ui) {
        let (dot, label, border) = match self.online {
            Some(true) => (OK, self.online_label.as_str(), ACCENT_DIM),
            Some(false) => (DANGER, self.online_label.as_str(), DANGER),
            None => (MUTED, self.online_label.as_str(), BORDER),
        };
        Frame::NONE
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, border))
            .corner_radius(CornerRadius::same(RADIUS))
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, dot);
                    ui.add_space(6.0);
                    ui.label(RichText::new(label).size(12.0).color(TEXT));
                });
            });
    }

    fn draw_form(&mut self, ui: &mut Ui, w: f32) {
        const INSET: f32 = 14.0;
        let card_h = 196.0_f32;
        let (card, _) = ui.allocate_exact_size(Vec2::new(w, card_h), Sense::hover());
        ui.painter().rect(
            card,
            CornerRadius::same(10),
            PANEL,
            Stroke::new(1.0_f32, BORDER),
            egui::StrokeKind::Inside,
        );

        // Absolute rows inside the card — same X/width for both fields.
        let x = card.min.x + INSET;
        let row_w = (card.width() - INSET * 2.0).floor();
        let mut y = card.min.y + INSET;

        // Username label
        {
            let label_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(row_w, 18.0));
            let mut label_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(label_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            field_label(&mut label_ui, "ชื่อผู้ใช้", true);
            y += 24.0;
        }

        // Username field
        {
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(row_w, CTRL_H));
            paint_field_chrome(ui, rect);
            let mut row = field_child(ui, rect);
            let edit_w = row_w - 20.0;
            row.add_sized(
                [edit_w, CTRL_H - 8.0],
                TextEdit::singleline(&mut self.username)
                    .desired_width(edit_w)
                    .frame(false)
                    .hint_text(RichText::new("กรอกชื่อผู้ใช้จากใบเสร็จ").color(MUTED))
                    .text_color(TEXT)
                    .margin(Margin::symmetric(4, 8)),
            );
            y += CTRL_H + SPACE;
        }

        // Password label
        {
            let label_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(row_w, 18.0));
            let mut label_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(label_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            field_label(&mut label_ui, "รหัสผ่าน", true);
            y += 24.0;
        }

        // Password field — identical outer width to username
        {
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(row_w, CTRL_H));
            paint_field_chrome(ui, rect);
            let mut row = field_child(ui, rect);
            row.spacing_mut().item_spacing.x = 0.0;
            let eye_w = 56.0_f32;
            let edit_w = (row_w - 20.0 - eye_w).max(60.0);
            let mut edit = TextEdit::singleline(&mut self.password)
                .desired_width(edit_w)
                .frame(false)
                .hint_text(RichText::new("กรอกรหัสผ่าน").color(MUTED))
                .text_color(TEXT)
                .margin(Margin::symmetric(4, 8));
            if !self.show_password {
                edit = edit.password(true);
            }
            row.add_sized([edit_w, CTRL_H - 8.0], edit);
            let eye = if self.show_password { "ซ่อน" } else { "แสดง" };
            if row
                .add_sized(
                    [eye_w, CTRL_H - 8.0],
                    egui::Button::new(RichText::new(eye).size(12.0).color(MUTED))
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .frame(false),
                )
                .on_hover_text(if self.show_password {
                    "ซ่อนรหัสผ่าน"
                } else {
                    "แสดงรหัสผ่าน"
                })
                .clicked()
            {
                self.show_password = !self.show_password;
            }
        }
    }

    fn draw_actions(&mut self, ui: &mut Ui, w: f32) {
        let enabled = !self.busy;
        let col = ((w - COL_GAP) * 0.5).floor().max(120.0);
        let size = Vec2::new(col, CTRL_H);

        let (r1, _) = ui.allocate_exact_size(Vec2::new(w, CTRL_H), Sense::hover());
        let l1 = egui::Rect::from_min_size(r1.min, size);
        let r1b = egui::Rect::from_min_size(r1.min + Vec2::new(col + COL_GAP, 0.0), size);
        let connect = ui
            .new_child(
                egui::UiBuilder::new()
                    .max_rect(l1)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .add_enabled(
                enabled,
                filled_btn(
                    "บันทึกแล้วต่อเน็ตเลย",
                    ACCENT,
                    Color32::from_rgb(0x07, 0x11, 0x1F),
                )
                .min_size(size),
            )
            .clicked();
        let save = ui
            .new_child(
                egui::UiBuilder::new()
                    .max_rect(r1b)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .add_enabled(
                enabled,
                outline_btn("บันทึกอย่างเดียว", ACCENT).min_size(size),
            )
            .clicked();

        ui.add_space(COL_GAP);

        let (r2, _) = ui.allocate_exact_size(Vec2::new(w, CTRL_H), Sense::hover());
        let l2 = egui::Rect::from_min_size(r2.min, size);
        let r2b = egui::Rect::from_min_size(r2.min + Vec2::new(col + COL_GAP, 0.0), size);
        let clear_form = ui
            .new_child(
                egui::UiBuilder::new()
                    .max_rect(l2)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .add_enabled(enabled, outline_btn("ลบรหัส", MUTED).min_size(size))
            .on_hover_text("ล้างช่องกรอกบนหน้าจอ")
            .clicked();
        let del_stored = ui
            .new_child(
                egui::UiBuilder::new()
                    .max_rect(r2b)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .add_enabled(
                enabled && self.has_stored,
                outline_btn("ลบรหัสที่เก็บไว้แล้ว", DANGER).min_size(size),
            )
            .on_hover_text("ลบรหัสออกจากเครื่องนี้")
            .clicked();

        if connect {
            self.action_save(true);
        }
        if save {
            self.action_save(false);
        }
        if clear_form {
            self.username.clear();
            self.password.clear();
            self.set_status(true, "ล้างช่องกรอกแล้ว");
        }
        if del_stored {
            self.confirm = ConfirmKind::DeleteStored;
        }
    }

    fn draw_feedback(&self, ui: &mut Ui, w: f32) {
        if self.status.is_empty() && !self.busy {
            return;
        }
        let (bg, fg, border) = if self.busy {
            (PANEL, MUTED, BORDER)
        } else if self.status_ok {
            (Color32::from_rgb(0x0A, 0x28, 0x24), OK, ACCENT_DIM)
        } else {
            (DANGER_SOFT, DANGER, DANGER)
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, CTRL_H), Sense::hover());
        ui.painter().rect(
            rect,
            CornerRadius::same(RADIUS),
            bg,
            Stroke::new(1.0_f32, border),
            egui::StrokeKind::Inside,
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(Vec2::new(12.0, 8.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if self.busy {
            child.spinner();
            child.add_space(8.0);
            child.label(RichText::new("กำลังดำเนินการ...").size(13.0).color(MUTED));
        } else {
            child.label(RichText::new(&self.status).size(13.0).color(fg));
        }
    }

    fn draw_footer(&self, ui: &mut Ui) {
        ui.label(
            RichText::new(format!("โดย {APP_CREATOR} · {APP_DISPLAY_NAME_EN} · v{APP_VERSION}"))
                .size(11.0)
                .color(MUTED),
        );
    }

    fn draw_confirm_delete(&mut self, ctx: egui::Context) {
        egui::Window::new("ยืนยันการลบ")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::NONE
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(16, 14)),
            )
            .show(&ctx, |ui| {
                ui.set_min_width(320.0);
                ui.label(
                    RichText::new("ลบรหัสที่เก็บไว้บนเครื่องนี้?")
                        .size(15.0)
                        .color(TEXT)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("หลังจากลบแล้ว ต้องใส่รหัสใหม่เมื่อซื้อเน็ตเดือนหน้า")
                        .size(12.5)
                        .color(MUTED),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(filled_btn("ลบเลย", DANGER, TEXT).min_size(Vec2::new(100.0, 36.0)))
                        .clicked()
                    {
                        self.confirm = ConfirmKind::None;
                        self.action_delete_stored();
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(outline_btn("ยกเลิก", MUTED).min_size(Vec2::new(100.0, 36.0)))
                        .clicked()
                    {
                        self.confirm = ConfirmKind::None;
                    }
                });
            });
    }

    fn action_save(&mut self, then_login: bool) {
        let user = self.username.trim().to_string();
        if user.is_empty() {
            self.set_status(false, "กรุณากรอกชื่อผู้ใช้");
            return;
        }
        if self.password.is_empty() {
            self.set_status(false, "กรุณากรอกรหัสผ่าน");
            return;
        }

        self.busy = true;
        self.status.clear();
        let pass = self.password.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            if creds::store(&Credentials::new(&user, &pass)).is_err() {
                let _ = tx.send(JobResult::Err(
                    "บันทึกไม่สำเร็จ ลองปิดแล้วเปิดโปรแกรมใหม่".into(),
                ));
                return;
            }
            if !then_login {
                let _ = tx.send(JobResult::Saved);
                return;
            }
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                let _ = tx.send(JobResult::Err("เริ่มระบบไม่สำเร็จ ลองใหม่อีกครั้ง".into()));
                return;
            };
            let result = rt.block_on(async {
                let network = NetworkMonitor::new()
                    .map_err(|_| "เชื่อมต่อระบบเน็ตหอไม่ได้".to_string())?;
                MikroTikHotspotAuth::default()
                    .authenticate(network.client(), &Credentials::new(&user, &pass))
                    .await
                    .map_err(friendly_auth_error)?;
                Ok::<_, String>(())
            });
            match result {
                Ok(()) => {
                    let _ = tx.send(JobResult::LoginOk);
                }
                Err(e) => {
                    let _ = tx.send(JobResult::Err(format!(
                        "บันทึกไว้แล้ว แต่ยังต่อเน็ตไม่ได้ — {e}"
                    )));
                }
            }
        });
    }

    fn action_delete_stored(&mut self) {
        self.busy = true;
        self.status.clear();
        let tx = self.tx.clone();
        thread::spawn(move || match creds::delete() {
            Ok(()) => {
                let _ = tx.send(JobResult::DeletedStored);
            }
            Err(_) => {
                let _ = tx.send(JobResult::Err("ลบรหัสที่เก็บไว้ไม่สำเร็จ".into()));
            }
        });
    }
}

fn friendly_auth_error(err: AuthError) -> String {
    match err {
        AuthError::BadCredentials => "ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง".into(),
        AuthError::FalseSuccess => "ระบบหอตอบกลับไม่ชัด ลองใหม่อีกสักครู่".into(),
        AuthError::Transient(_) => "เน็ตหอยังไม่พร้อม รอแล้วลองใหม่".into(),
        AuthError::UserActionRequired => "ต้องยืนยันเพิ่มในเบราว์เซอร์".into(),
        AuthError::NotConfigured => "ยังไม่มีรหัสที่บันทึก".into(),
    }
}

fn field_label(ui: &mut Ui, text: &str, required: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(12.5).color(MUTED));
        if required {
            ui.label(RichText::new("*").size(12.5).color(REQUIRED));
        }
    });
    ui.add_space(6.0);
}

fn paint_field_chrome(ui: &Ui, rect: egui::Rect) {
    ui.painter().rect(
        rect,
        CornerRadius::same(RADIUS),
        FIELD,
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Inside,
    );
}

fn field_child(ui: &mut Ui, rect: egui::Rect) -> Ui {
    let inner = rect.shrink2(Vec2::new(10.0, 4.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center))
            .sense(Sense::click()),
    );
    child.set_clip_rect(inner);
    child
}

fn filled_btn(label: &str, fill: Color32, text: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_owned()).size(13.5).color(text).strong())
        .fill(fill)
        .corner_radius(CornerRadius::same(RADIUS))
        .stroke(Stroke::new(1.0_f32, fill))
}

fn outline_btn(label: &str, color: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_owned()).size(13.5).color(color).strong())
        .fill(Color32::TRANSPARENT)
        .corner_radius(CornerRadius::same(RADIUS))
        .stroke(Stroke::new(1.2_f32, color))
}

fn paint_mark(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(8), PANEL);
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0_f32, BORDER),
        egui::StrokeKind::Outside,
    );
    let c = rect.center();
    for r in [size * 0.14, size * 0.22, size * 0.30] {
        p.circle_stroke(c, r, Stroke::new(1.4_f32, ACCENT));
    }
}

fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, TextureOptions::LINEAR))
}

fn install_thai_fonts(ctx: &egui::Context) {
    let candidates = ["LeelawUI.ttf", "leelawad.ttf", "tahoma.ttf", "segoeui.ttf"];
    let fonts_dir =
        PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into()))
            .join("Fonts");
    for name in candidates {
        if let Ok(bytes) = std::fs::read(fonts_dir.join(name)) {
            let key = name.trim_end_matches(".ttf").to_string();
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, key.clone());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, key);
            ctx.set_fonts(fonts);
            return;
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.window_fill = BG;
    v.panel_fill = BG;
    v.extreme_bg_color = FIELD;
    v.widgets.inactive.bg_fill = PANEL;
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x12, 0x24, 0x38);
    v.widgets.active.bg_fill = ACCENT_DIM;
    v.selection.bg_fill = Color32::from_rgb(0x0A, 0x3A, 0x3A);
    v.hyperlink_color = BLUE;
    v.override_text_color = Some(TEXT);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 8.0);
    ctx.set_style(style);
}

fn load_window_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/window-icon.png"))
        .or_else(|_| {
            eframe::icon_data::from_png_bytes(include_bytes!("../assets/dormnet-icon-32.png"))
        })
        .ok()
}

pub fn run() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([460.0, 540.0])
        .with_min_inner_size([460.0, 540.0])
        .with_max_inner_size([460.0, 540.0])
        .with_resizable(false)
        .with_title(APP_DISPLAY_NAME_EN);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    eframe::run_native(
        APP_DISPLAY_NAME_EN,
        eframe::NativeOptions {
            viewport,
            centered: true,
            ..Default::default()
        },
        Box::new(|cc| {
            install_thai_fonts(&cc.egui_ctx);
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    )
}
