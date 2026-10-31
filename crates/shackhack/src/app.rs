use crate::audio::AudioOut;
use crate::theme::{self, CONTACT, PHOSPHOR, PHOSPHOR_DIM, PHOSPHOR_HOT, SCOPE, VOID};
use eframe::egui::{
    self, Align, Color32, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};
use shackhack_bands::{self, BandPreset, Demod, SweepMode};
use shackhack_dsp::colormap::db_to_color;
use shackhack_dsp::sweep::Mode;
use shackhack_dsp::vfo::{DemodKind, VFO_COUNT};
use shackhack_sdr::{Command, RadioHandle, SourceKind};

pub struct ShackHackApp {
    radio: RadioHandle,
    _audio: Option<AudioOut>,
    waterfall: Vec<Color32>,
    wf_w: usize,
    wf_h: usize,
    wf_tex: Option<egui::TextureHandle>,
    overview: Vec<f32>,
    preset_id: String,
    custom_min_mhz: f64,
    custom_max_mhz: f64,
    gain_db: f32,
    driver_note: Option<String>,
    driver_events: std::sync::mpsc::Receiver<String>,
}

impl ShackHackApp {
    pub fn new(cc: &eframe::CreationContext<'_>, kind: SourceKind) -> Self {
        theme::apply(&cc.egui_ctx);
        let radio = shackhack_sdr::start(kind);
        let audio = AudioOut::start(radio.audio.clone()).ok();
        Self {
            radio,
            _audio: audio,
            waterfall: Vec::new(),
            wf_w: 1024,
            wf_h: 280,
            wf_tex: None,
            overview: vec![-90.0; 1024],
            preset_id: "fm".into(),
            custom_min_mhz: 88.0,
            custom_max_mhz: 108.0,
            gain_db: 29.6,
            driver_note: None,
            driver_events: spawn_driver_check(kind),
        }
    }
}

fn spawn_driver_check(kind: SourceKind) -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    if kind != SourceKind::Rtl {
        drop(tx);
        return rx;
    }
    #[cfg(windows)]
    {
        std::thread::spawn(move || match crate::winusb::ensure_if_needed() {
            Ok(Some(msg)) => {
                let _ = tx.send(msg);
            }
            Ok(None) => {}
            Err(e) => {
                let _ = tx.send(format!("WinUSB check failed: {e}"));
            }
        });
    }
    #[cfg(not(windows))]
    {
        drop(tx);
    }
    rx
}

impl eframe::App for ShackHackApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        if let Ok(msg) = self.driver_events.try_recv() {
            self.driver_note = Some(msg);
        }
        let snap = self.radio.snapshot.lock().clone();

        if snap.spectrum.len() >= 32 {
            self.push_waterfall(
                &snap.spectrum,
                snap.floor_db - 5.0,
                snap.peak_db.max(snap.floor_db + 25.0),
            );
            self.accumulate_overview(&snap);
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            self.top_bar(ui, &snap);
        });
        egui::TopBottomPanel::bottom("bottom")
            .resizable(false)
            .show(ctx, |ui| {
                self.bottom_bar(ui, &snap);
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(note) = &self.driver_note {
                ui.label(
                    RichText::new(note)
                        .monospace()
                        .color(theme::AMBER),
                );
            }
            if let Some(err) = &snap.error {
                ui.colored_label(theme::ALERT, err);
                let driver_hint = err.contains("setup-driver") || err.contains("WinUSB");
                if driver_hint {
                    ui.label(
                        RichText::new(
                            "No kernel driver to compile. Bind Microsoft WinUSB to Bulk-In Interface 0.",
                        )
                        .monospace()
                        .color(PHOSPHOR_DIM),
                    );
                }
                #[cfg(windows)]
                if driver_hint {
                    if ui.button("Install WinUSB package…").clicked() {
                        match crate::winusb::start_installer() {
                            Ok(p) => {
                                self.driver_note = Some(format!(
                                    "Launched {} — pick Interface 0, WinUSB.",
                                    p.display()
                                ));
                            }
                            Err(e) => self.driver_note = Some(e.to_string()),
                        }
                    }
                    ui.label("or: shackhack --setup-driver");
                }
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SLS SPIRIT BOX")
                        .monospace()
                        .size(11.0)
                        .color(PHOSPHOR_DIM),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{}   {}   LO {:.3} MHz",
                        snap.device_label,
                        if snap.hf_ok {
                            "HF DEPTH"
                        } else {
                            "FLOOR 24 MHz"
                        },
                        snap.lo_hz / 1e6
                    ))
                    .monospace()
                    .size(12.0)
                    .color(PHOSPHOR_DIM),
                );
            });
            self.band_overview(ui, &snap);
            ui.add_space(4.0);
            let spec_h = 110.0;
            let spec_rect = ui
                .allocate_exact_size(
                    Vec2::new(ui.available_width(), spec_h),
                    Sense::click_and_drag(),
                )
                .0;
            self.paint_spectrum(ui, spec_rect, &snap);
            ui.add_space(4.0);
            let wf_h = ui.available_height().max(160.0);
            let wf_rect = ui
                .allocate_exact_size(
                    Vec2::new(ui.available_width(), wf_h),
                    Sense::click_and_drag(),
                )
                .0;
            self.paint_waterfall(ui, wf_rect, &snap, ctx);
        });
    }
}

impl ShackHackApp {
    fn top_bar(&mut self, ui: &mut Ui, snap: &shackhack_sdr::Snapshot) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("SHACKHACK")
                    .strong()
                    .size(18.0)
                    .color(PHOSPHOR)
                    .monospace(),
            );
            ui.label(
                RichText::new("SLS")
                    .monospace()
                    .size(12.0)
                    .color(PHOSPHOR_DIM),
            );
            ui.separator();
            theme::live_badge(ui, snap.sweeping, ui.input(|i| i.time));
            let sweep_label = if snap.sweeping { "HOLD" } else { "SWEEP" };
            if ui
                .add(egui::Button::new(
                    RichText::new(sweep_label).monospace().strong(),
                ))
                .clicked()
            {
                self.radio.send(Command::SetSweeping(!snap.sweeping));
            }
            ui.separator();
            let display = snap.vfos[0].display_hz;
            ui.label(
                RichText::new(format_hz(display))
                    .font(FontId::monospace(22.0))
                    .color(PHOSPHOR),
            );
            ui.label(
                RichText::new(kind_str(snap.demod))
                    .monospace()
                    .size(16.0)
                    .color(PHOSPHOR_HOT),
            );
            ui.separator();
            ui.label("Band");
            egui::ComboBox::from_id_salt("band")
                .selected_text(
                    shackhack_bands::preset_by_id(&self.preset_id)
                        .map(|p| p.name)
                        .unwrap_or("Custom"),
                )
                .show_ui(ui, |ui| {
                    for p in shackhack_bands::available_presets(snap.hf_ok) {
                        if ui
                            .selectable_label(self.preset_id == p.id, p.name)
                            .clicked()
                        {
                            self.apply_preset(p, snap.mode);
                        }
                    }
                });
            ui.separator();
            ui.label("Mode");
            for m in SweepMode::all() {
                let on = matches!(
                    (snap.mode, m),
                    (Mode::Bounce, SweepMode::Bounce)
                        | (Mode::Forward, SweepMode::Forward)
                        | (Mode::Backward, SweepMode::Backward)
                        | (Mode::Random, SweepMode::Random)
                );
                if ui.selectable_label(on, m.as_str()).clicked() {
                    self.radio.send(Command::SetMode(*m));
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Gain").strong());
            let max_g = snap.gains.last().copied().unwrap_or(496) as f32 / 10.0;
            let min_g = snap.gains.first().copied().unwrap_or(0) as f32 / 10.0;
            let slider = egui::Slider::new(&mut self.gain_db, min_g..=max_g)
                .suffix(" dB")
                .show_value(true)
                .min_decimals(1);
            let resp = ui.add(slider);
            if resp.changed() {
                self.radio
                    .send(Command::SetGain((self.gain_db * 10.0).round() as i32));
            } else if !resp.dragged() {
                self.gain_db = snap.gain_tenth_db as f32 / 10.0;
            }
            if snap.clip {
                ui.colored_label(theme::ALERT, "OVERLOAD — drop gain");
            }
            ui.separator();
            ui.label("Dwell");
            let mut dwell = snap.dwell_ms as f32;
            if ui
                .add(egui::Slider::new(&mut dwell, 10.0..=200.0).suffix(" ms"))
                .changed()
            {
                self.radio.send(Command::SetDwell(dwell as u32));
            }
            ui.label("Step");
            let mut step = snap.step_hz as f32 / 1000.0;
            if ui
                .add(egui::Slider::new(&mut step, 5.0..=400.0).suffix(" kHz"))
                .changed()
            {
                self.radio.send(Command::SetStep((step * 1000.0) as u64));
            }
            ui.separator();
            ui.label("Demod");
            for d in Demod::all() {
                let on = matches!(
                    (snap.demod, d),
                    (DemodKind::Wfm, Demod::Wfm)
                        | (DemodKind::Nfm, Demod::Nfm)
                        | (DemodKind::Am, Demod::Am)
                );
                if ui.selectable_label(on, d.as_str()).clicked() {
                    self.radio.send(Command::SetDemod(*d));
                }
            }
            ui.separator();
            ui.label(RichText::new("SIG").monospace().color(PHOSPHOR_DIM));
            theme::emf_bar(ui, snap.audio_rms);
            ui.label("Vol");
            let mut vol = snap.volume;
            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.2)).changed() {
                self.radio.send(Command::SetVolume(vol));
            }
            let mute_l = if snap.mute { "Unmute" } else { "Mute" };
            if ui.button(mute_l).clicked() {
                self.radio.send(Command::SetMute(!snap.mute));
            }
        });
        ui.add_space(2.0);
    }

    fn bottom_bar(&mut self, ui: &mut Ui, snap: &shackhack_sdr::Snapshot) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("CONTACTS").monospace().color(PHOSPHOR_DIM));
            for i in 0..VFO_COUNT {
                let v = &snap.vfos[i];
                let name = ['A', 'B', 'C', 'D'][i];
                ui.separator();
                let mut on = v.enabled;
                if ui
                    .checkbox(
                        &mut on,
                        RichText::new(format!("ENT {name}"))
                            .monospace()
                            .color(CONTACT[i]),
                    )
                    .changed()
                {
                    self.radio.send(Command::EnableVfo { index: i, on });
                }
                ui.monospace(format_hz(if i == 0 { v.display_hz } else { v.freq_hz }));
                if !v.in_window {
                    ui.colored_label(PHOSPHOR_DIM, "off-field");
                }
                if i > 0 {
                    let mut mix = v.mix;
                    if ui
                        .add(egui::Slider::new(&mut mix, 0.0..=1.0).max_decimals(2))
                        .changed()
                    {
                        self.radio.send(Command::SetMix { index: i, mix });
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Custom range MHz");
            ui.add(egui::DragValue::new(&mut self.custom_min_mhz).speed(0.1));
            ui.label("–");
            ui.add(egui::DragValue::new(&mut self.custom_max_mhz).speed(0.1));
            if ui.button("Apply range").clicked() {
                let min = (self.custom_min_mhz * 1e6) as u64;
                let max = (self.custom_max_mhz * 1e6) as u64;
                self.radio.send(Command::SetRange {
                    min_hz: min,
                    max_hz: max,
                });
                self.preset_id = "custom".into();
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(
                        "click sweep field to park ENT B  ·  receive only  ·  analog ghost box",
                    )
                    .monospace()
                    .size(11.0)
                    .color(PHOSPHOR_DIM),
                );
            });
        });
    }

    fn apply_preset(&mut self, p: &BandPreset, mode: Mode) {
        self.preset_id = p.id.to_string();
        self.custom_min_mhz = p.start_hz as f64 / 1e6;
        self.custom_max_mhz = p.end_hz as f64 / 1e6;
        self.radio.send(Command::ApplyPreset {
            start_hz: p.start_hz,
            end_hz: p.end_hz,
            step_hz: p.step_hz,
            dwell_ms: p.dwell_ms,
            demod: p.demod,
            mode: match mode {
                Mode::Bounce => SweepMode::Bounce,
                Mode::Forward => SweepMode::Forward,
                Mode::Backward => SweepMode::Backward,
                Mode::Random => SweepMode::Random,
            },
        });
        self.overview.fill(-90.0);
    }

    fn band_overview(&mut self, ui: &mut Ui, snap: &shackhack_sdr::Snapshot) {
        let (rect, _r) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, VOID);
        let n = self.overview.len().max(1);
        let w = rect.width();
        let h = rect.height();
        for (i, db) in self.overview.iter().enumerate() {
            let x = rect.left() + i as f32 / n as f32 * w;
            let c = db_to_color(*db, snap.floor_db - 10.0, snap.peak_db + 5.0);
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x, rect.top()),
                    Vec2::new((w / n as f32).max(1.0), h),
                ),
                0.0,
                Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255),
            );
        }
        let span = (snap.max_hz - snap.min_hz).max(1) as f64;
        let x0 = ((snap.lo_hz - snap.sample_rate * 0.5) - snap.min_hz as f64) / span;
        let x1 = ((snap.lo_hz + snap.sample_rate * 0.5) - snap.min_hz as f64) / span;
        let win = Rect::from_min_max(
            Pos2::new(rect.left() + x0 as f32 * w, rect.top()),
            Pos2::new(rect.left() + x1 as f32 * w, rect.bottom()),
        );
        painter.rect_stroke(
            win,
            0.0,
            Stroke::new(1.5_f32, PHOSPHOR),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.left_top() + Vec2::new(10.0, 6.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{:.3} – {:.3} MHz",
                snap.min_hz as f64 / 1e6,
                snap.max_hz as f64 / 1e6
            ),
            FontId::monospace(11.0),
            PHOSPHOR,
        );
        theme::hud_frame(&painter, rect, "");
    }

    fn paint_spectrum(&self, ui: &mut Ui, rect: Rect, snap: &shackhack_sdr::Snapshot) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, SCOPE);
        theme::scanlines(&painter, rect);
        if snap.spectrum.len() >= 8 {
            let floor = snap.floor_db - 8.0;
            let ceil = snap.peak_db.max(floor + 20.0);
            let n = snap.spectrum.len();
            let mut pts = Vec::with_capacity(n);
            for (i, db) in snap.spectrum.iter().enumerate() {
                let x = rect.left() + i as f32 / (n - 1) as f32 * rect.width();
                let t = ((*db - floor) / (ceil - floor)).clamp(0.0, 1.0);
                let y = rect.bottom() - t * rect.height();
                pts.push(Pos2::new(x, y));
            }
            painter.add(egui::Shape::line(pts, Stroke::new(1.4_f32, PHOSPHOR)));
            self.overlay_vfos(&painter, rect, snap, true);
        }
        theme::hud_frame(&painter, rect, "SWEEP FIELD");
    }

    fn paint_waterfall(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        snap: &shackhack_sdr::Snapshot,
        ctx: &egui::Context,
    ) {
        if self.waterfall.len() == self.wf_w * self.wf_h {
            let img = egui::ColorImage {
                size: [self.wf_w, self.wf_h],
                pixels: self.waterfall.clone(),
                source_size: Vec2::new(self.wf_w as f32, self.wf_h as f32),
            };
            match &mut self.wf_tex {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => {
                    self.wf_tex = Some(ctx.load_texture("wf", img, egui::TextureOptions::LINEAR));
                }
            }
        }
        if let Some(tex) = &self.wf_tex {
            ui.painter_at(rect).image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui.painter_at(rect).rect_filled(rect, 0.0, VOID);
        }
        let painter = ui.painter_at(rect);
        theme::scanlines(&painter, rect);
        self.overlay_vfos(&painter, rect, snap, false);
        theme::hud_frame(&painter, rect, "NV WATERFALL");

        if ui.input(|i| i.pointer.button_released(PointerButton::Primary)) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if rect.contains(pos) {
                    if let Some(hz) = x_to_hz(pos.x, rect, snap) {
                        if snap.sweeping {
                            self.radio.send(Command::ParkVfo {
                                index: 1,
                                hz: hz as u64,
                            });
                        } else {
                            self.radio.send(Command::TuneA(hz as u64));
                        }
                    }
                }
            }
        }
    }

    fn overlay_vfos(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        snap: &shackhack_sdr::Snapshot,
        fill: bool,
    ) {
        for (i, v) in snap.vfos.iter().enumerate() {
            if !v.enabled {
                continue;
            }
            let hz = if i == 0 { v.display_hz } else { v.freq_hz };
            let x = hz_to_x(hz, rect, snap);
            let bw = v.demod.passband_hz();
            let x0 = hz_to_x(hz - bw * 0.5, rect, snap);
            let x1 = hz_to_x(hz + bw * 0.5, rect, snap);
            let color = CONTACT[i];
            if fill {
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x1, rect.bottom())),
                    0.0,
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28),
                );
            }
            painter.vline(x, rect.y_range(), Stroke::new(1.5_f32, color));
            let scale = if fill { 0.85 } else { 0.65 };
            theme::sls_figure(painter, x, rect.top() + 2.0, color, scale);
            let name = ['A', 'B', 'C', 'D'][i];
            painter.text(
                Pos2::new(x + 8.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                format!("ENT {name}"),
                FontId::monospace(10.0),
                color,
            );
        }
    }

    fn push_waterfall(&mut self, db: &[f32], floor: f32, ceil: f32) {
        let w = self.wf_w;
        let h = self.wf_h;
        if self.waterfall.len() != w * h {
            self.waterfall = vec![Color32::BLACK; w * h];
        }
        let mut row = vec![Color32::BLACK; w];
        for (x, pixel) in row.iter_mut().enumerate() {
            let src = x * db.len() / w;
            let c = db_to_color(db[src.min(db.len() - 1)], floor, ceil);
            *pixel = Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255);
        }
        self.waterfall.copy_within(0..(h - 1) * w, w);
        self.waterfall[..w].copy_from_slice(&row);
    }

    fn accumulate_overview(&mut self, snap: &shackhack_sdr::Snapshot) {
        let n = self.overview.len();
        let span = (snap.max_hz - snap.min_hz).max(1) as f64;
        let sr = snap.sample_rate;
        let lo = snap.lo_hz;
        let bins = snap.spectrum.len();
        for (i, db) in snap.spectrum.iter().enumerate() {
            let freq = lo - sr * 0.5 + sr * i as f64 / bins as f64;
            if freq < snap.min_hz as f64 || freq > snap.max_hz as f64 {
                continue;
            }
            let idx = ((freq - snap.min_hz as f64) / span * n as f64) as usize;
            if idx < n {
                self.overview[idx] = self.overview[idx].max(*db);
                self.overview[idx] *= 0.999; // slow decay
            }
        }
    }
}

fn format_hz(hz: f64) -> String {
    let hz = hz.max(0.0) as u64;
    format!(
        "{:03}.{:03}.{:03}",
        hz / 1_000_000,
        (hz / 1000) % 1000,
        hz % 1000
    )
}

fn kind_str(d: DemodKind) -> &'static str {
    match d {
        DemodKind::Wfm => "WFM",
        DemodKind::Nfm => "NFM",
        DemodKind::Am => "AM",
    }
}

fn hz_to_x(hz: f64, rect: Rect, snap: &shackhack_sdr::Snapshot) -> f32 {
    let start = snap.lo_hz - snap.sample_rate * 0.5;
    let t = ((hz - start) / snap.sample_rate).clamp(0.0, 1.0) as f32;
    rect.left() + t * rect.width()
}

fn x_to_hz(x: f32, rect: Rect, snap: &shackhack_sdr::Snapshot) -> Option<f64> {
    if rect.width() <= 1.0 {
        return None;
    }
    let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
    Some(snap.lo_hz - snap.sample_rate * 0.5 + t * snap.sample_rate)
}
