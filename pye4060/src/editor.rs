//! Editor: a flat, dark UI laid out like the 4060 dual frame.
//!
//! Each channel module keeps the hardware arrangement - attenuation meter on
//! top, NOISEGATE THRESHOLD (with the attenuation switch mounted
//! concentrically, as on the unit) beside THRESHOLD, then DECAY TIME and
//! COMPRESSION RATIO - drawn as flat vector controls. The meter keeps the
//! Pye scale (0-24 dB, compressed at both ends by the saturated meter
//! amplifier) with moving-coil needle ballistics.

use std::sync::Arc;

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use truce::prelude::*;
use truce_egui::{EditorUi, EguiEditor};

use crate::{Pye4060Params, Pye4060ParamsParamId as P};

const W: f32 = 920.0;
const H: f32 = 560.0;

mod pal {
    use egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x1b, 0x1b, 0x1b);
    pub const HEADER: Color32 = Color32::from_rgb(0x14, 0x14, 0x14);
    pub const PANEL: Color32 = Color32::from_rgb(0x25, 0x25, 0x25);
    pub const PANEL_EDGE: Color32 = Color32::from_rgb(0x31, 0x31, 0x31);
    pub const FACE: Color32 = Color32::from_rgb(0x10, 0x10, 0x10);
    pub const BODY: Color32 = Color32::from_rgb(0x34, 0x34, 0x34);
    pub const BODY_EDGE: Color32 = Color32::from_rgb(0x45, 0x45, 0x45);
    pub const TRACK: Color32 = Color32::from_rgb(0x3b, 0x3b, 0x3b);
    pub const TEXT: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xdc);
    pub const DIM: Color32 = Color32::from_rgb(0x8c, 0x8c, 0x8c);
    pub const FAINT: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
    /// The orange of the noise-gate scale on the Pye front panel.
    pub const ORANGE: Color32 = Color32::from_rgb(0xf2, 0x8c, 0x28);
    pub const WARM: Color32 = Color32::from_rgb(0xf4, 0xe9, 0xd8);
    pub const GREEN: Color32 = Color32::from_rgb(0x7c, 0xc5, 0x5a);
    pub const YELLOW: Color32 = Color32::from_rgb(0xe8, 0xc5, 0x47);
    pub const RED: Color32 = Color32::from_rgb(0xe0, 0x4a, 0x3f);
}

/// Title offset above the centre of the top-row controls.
const ROW1_TITLE: f32 = 64.0;

const START: f32 = std::f32::consts::FRAC_PI_4 * 3.0;
const SWEEP: f32 = std::f32::consts::FRAC_PI_2 * 3.0;

fn id(p: P) -> u32 {
    p.into()
}

// ---------------------------------------------------------------------------
// Parameter plumbing

fn info(state: &PluginContext<Pye4060Params>, pid: u32) -> Option<truce::params::ParamInfo> {
    state.params().param_infos().into_iter().find(|i| i.id == pid)
}

fn steps_of(state: &PluginContext<Pye4060Params>, pid: u32) -> Option<u32> {
    info(state, pid).and_then(|i| i.range.step_count()).map(std::num::NonZeroU32::get)
}

fn set_now(state: &PluginContext<Pye4060Params>, pid: u32, v: f32) {
    state.begin_edit(pid);
    state.set_param(pid, f64::from(v.clamp(0.0, 1.0)));
    state.end_edit(pid);
}

fn reset_default(state: &PluginContext<Pye4060Params>, pid: u32) {
    if let Some(i) = info(state, pid) {
        set_now(state, pid, i.range.normalize(i.default_plain) as f32);
    }
}

/// Vertical drag (Shift = fine), wheel, double-click to default. `snap`
/// quantises a continuous parameter (released by holding Shift).
fn drag_param(ui: &mut Ui, state: &PluginContext<Pye4060Params>, pid: u32, rect: Rect, snap: Option<u32>, enabled: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(("pye-ctl", pid)), if enabled { Sense::click_and_drag() } else { Sense::hover() });
    if !enabled {
        return resp;
    }
    let steps = steps_of(state, pid).or(snap);
    let shift = ui.input(|i| i.modifiers.shift);
    let acc_id = ui.id().with(("pye-acc", pid));
    let cur = state.get_param(pid);

    if resp.double_clicked() {
        reset_default(state, pid);
        return resp;
    }
    if resp.drag_started() {
        state.begin_edit(pid);
        ui.memory_mut(|m| m.data.insert_temp(acc_id, cur));
    }
    if resp.dragged() {
        let d = resp.drag_delta();
        let speed = if shift { 1.0 / 900.0 } else { 1.0 / 200.0 };
        let mut acc: f32 = ui.memory(|m| m.data.get_temp(acc_id)).unwrap_or(cur);
        acc = (acc + (d.x - d.y) * speed).clamp(0.0, 1.0);
        ui.memory_mut(|m| m.data.insert_temp(acc_id, acc));
        let quant = if snap.is_some() && shift { None } else { steps };
        let v = match quant {
            Some(n) => (acc * n as f32).round() / n as f32,
            None => acc,
        };
        if (v - cur).abs() > 1e-6 {
            state.set_param(pid, f64::from(v));
        }
    }
    if resp.drag_stopped() {
        state.end_edit(pid);
        ui.memory_mut(|m| m.data.remove_temp::<f32>(acc_id));
    }
    if resp.hovered() {
        // Wheel: one step (or 1 %) per notch-sized chunk of scrolling.
        let scroll_id = ui.id().with(("pye-scroll", pid));
        let mut acc: f32 = ui.memory(|m| m.data.get_temp(scroll_id)).unwrap_or(0.0);
        acc += ui.input(|i| i.smooth_scroll_delta.y);
        if acc.abs() >= 24.0 {
            let step = steps.map_or(0.01, |n| 1.0 / n as f32);
            set_now(state, pid, cur + step * acc.signum());
            acc = 0.0;
        }
        ui.memory_mut(|m| m.data.insert_temp(scroll_id, acc));
    }
    resp
}

// ---------------------------------------------------------------------------
// Painting helpers

fn arc(p: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    let n = ((a1 - a0).abs() * r / 4.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(egui::Shape::line(pts, stroke));
}

fn sk(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

fn dir(a: f32) -> egui::Vec2 {
    vec2(a.cos(), a.sin())
}

fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) {
    p.text(pos, align, s, FontId::proportional(size), color);
}

fn mono(p: &Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) {
    p.text(pos, align, s, FontId::monospace(size), color);
}

fn dim_if(c: Color32, enabled: bool) -> Color32 {
    if enabled { c } else { c.gamma_multiply(0.35) }
}

struct KnobLook {
    accent: Color32,
    bipolar: bool,
    enabled: bool,
    hot: bool,
    /// Continuous controls show a value arc; switches show detents instead.
    value_arc: bool,
}

fn paint_knob(p: &Painter, c: Pos2, r: f32, v: f32, look: &KnobLook) {
    let accent = dim_if(look.accent, look.enabled);
    p.circle_filled(c, r, if look.hot { Color32::from_rgb(0x3b, 0x3b, 0x3b) } else { pal::BODY });
    p.circle_stroke(c, r, sk(1.0, pal::BODY_EDGE));
    let ar = r + 6.0;
    if !look.value_arc {
        let a = START + SWEEP * v;
        p.line_segment([c + dir(a) * (r * 0.2), c + dir(a) * (r * 0.86)], sk(2.5, dim_if(pal::WARM, look.enabled)));
        return;
    }
    arc(p, c, ar, START, START + SWEEP, sk(3.0, pal::TRACK));
    let (a0, a1) = if look.bipolar {
        let mid = START + SWEEP * 0.5;
        let a = START + SWEEP * v;
        (mid.min(a), mid.max(a))
    } else {
        (START, START + SWEEP * v)
    };
    if a1 - a0 > 0.005 {
        arc(p, c, ar, a0, a1, sk(3.0, accent));
    }
    let a = START + SWEEP * v;
    p.line_segment([c + dir(a) * (r * 0.2), c + dir(a) * (r * 0.86)], sk(2.5, dim_if(pal::WARM, look.enabled)));
}

/// Detent ticks and labels around a stepped knob.
fn paint_scale(p: &Painter, c: Pos2, r: f32, labels: &[&str], active: usize, enabled: bool) {
    let n = labels.len().max(2) - 1;
    for (i, l) in labels.iter().enumerate() {
        let a = START + SWEEP * i as f32 / n as f32;
        let on = i == active;
        p.circle_filled(c + dir(a) * (r + 6.0), if on { 2.6 } else { 1.6 }, dim_if(if on { pal::ORANGE } else { pal::FAINT }, enabled));
        let col = dim_if(if on { pal::TEXT } else { pal::DIM }, enabled);
        text(p, c + dir(a) * (r + 19.0), Align2::CENTER_CENTER, l, 10.5, col);
    }
}

fn title(p: &Painter, pos: Pos2, s: &str, enabled: bool) {
    text(p, pos, Align2::CENTER_BOTTOM, s, 10.5, dim_if(pal::DIM, enabled));
}

// ---------------------------------------------------------------------------
// Controls

#[allow(clippy::too_many_arguments)]
fn knob(
    ui: &mut Ui,
    state: &PluginContext<Pye4060Params>,
    pid: u32,
    c: Pos2,
    r: f32,
    name: &str,
    accent: Color32,
    bipolar: bool,
    snap: Option<u32>,
    enabled: bool,
    title_gap: Option<f32>,
) {
    let rect = Rect::from_center_size(c, vec2(2.0 * r + 24.0, 2.0 * r + 24.0));
    let resp = drag_param(ui, state, pid, rect, snap, enabled);
    let v = state.get_param(pid);
    let p = ui.painter();
    paint_knob(p, c, r, v, &KnobLook { accent, bipolar, enabled, hot: resp.hovered() || resp.dragged(), value_arc: true });
    title(p, c - vec2(0.0, title_gap.unwrap_or(r + 15.0)), name, enabled);
    mono(p, c + vec2(0.0, r + 14.0), Align2::CENTER_TOP, &state.format_param(pid), 11.0, dim_if(pal::TEXT, enabled));
}

#[allow(clippy::too_many_arguments)]
fn stepped(
    ui: &mut Ui,
    state: &PluginContext<Pye4060Params>,
    pid: u32,
    c: Pos2,
    r: f32,
    name: &str,
    labels: &[&str],
    enabled: bool,
) {
    let n = labels.len() - 1;
    let rect = Rect::from_center_size(c, vec2(2.0 * r + 24.0, 2.0 * r + 24.0));
    let resp = drag_param(ui, state, pid, rect, None, enabled);
    // Clicking a scale label jumps straight to that position.
    for (i, _) in labels.iter().enumerate() {
        let a = START + SWEEP * i as f32 / n as f32;
        let lr = Rect::from_center_size(c + dir(a) * (r + 19.0), vec2(26.0, 16.0));
        let lresp = ui.interact(lr, ui.id().with(("pye-step", pid, i)), if enabled { Sense::click() } else { Sense::hover() });
        if enabled && lresp.clicked() {
            set_now(state, pid, i as f32 / n as f32);
        }
    }
    let v = state.get_param(pid);
    let active = (v * n as f32).round() as usize;
    let p = ui.painter();
    paint_knob(p, c, r, v, &KnobLook { accent: pal::ORANGE, bipolar: false, enabled, hot: resp.hovered() || resp.dragged(), value_arc: false });
    paint_scale(p, c, r, labels, active, enabled);
    title(p, c - vec2(0.0, r + 30.0), name, enabled);
}

/// NOISEGATE THRESHOLD knob with the attenuation switch mounted
/// concentrically inside it, as on the 845751/01 front panel.
fn gate_control(ui: &mut Ui, state: &PluginContext<Pye4060Params>, thr: u32, att: u32, c: Pos2, enabled: bool) {
    let (ro, ri) = (29.0, 14.0);
    let outer = drag_param(ui, state, thr, Rect::from_center_size(c, vec2(2.0 * ro + 24.0, 2.0 * ro + 24.0)), None, enabled);
    let inner = drag_param(ui, state, att, Rect::from_center_size(c, vec2(2.0 * ri + 4.0, 2.0 * ri + 4.0)), None, enabled);
    let vt = state.get_param(thr);
    let va = state.get_param(att);
    let active = va > 0.01;
    let p = ui.painter();
    let ring = if outer.hovered() || outer.dragged() { Color32::from_rgb(0x3b, 0x3b, 0x3b) } else { pal::BODY };
    p.circle_filled(c, ro, ring);
    p.circle_stroke(c, ro, sk(1.0, pal::BODY_EDGE));
    arc(p, c, ro + 6.0, START, START + SWEEP, sk(3.0, pal::TRACK));
    let orange = dim_if(if active { pal::ORANGE } else { pal::ORANGE.gamma_multiply(0.45) }, enabled);
    arc(p, c, ro + 6.0, START, START + SWEEP * vt, sk(3.0, orange));
    let a = START + SWEEP * vt;
    p.line_segment([c + dir(a) * (ri + 4.0), c + dir(a) * (ro - 3.0)], sk(2.5, dim_if(pal::WARM, enabled)));
    // Inner attenuation switch (6 positions, 0-10 dB).
    let hot_in = inner.hovered() || inner.dragged();
    p.circle_filled(c, ri, if hot_in { Color32::from_rgb(0x58, 0x58, 0x58) } else { Color32::from_rgb(0x4a, 0x4a, 0x4a) });
    for i in 0..6 {
        let ang = START + SWEEP * i as f32 / 5.0;
        let on = (va * 5.0).round() as usize == i;
        p.circle_filled(c + dir(ang) * (ri - 3.5), if on { 1.9 } else { 1.0 }, dim_if(if on { pal::ORANGE } else { pal::FAINT }, enabled));
    }
    let aa = START + SWEEP * va;
    p.line_segment([c, c + dir(aa) * (ri - 6.0)], sk(2.0, dim_if(pal::WARM, enabled)));

    text(p, c + dir(START) * (ro + 18.0), Align2::CENTER_CENTER, "-50", 10.0, dim_if(pal::ORANGE.gamma_multiply(0.8), enabled));
    text(p, c + dir(START + SWEEP) * (ro + 18.0), Align2::CENTER_CENTER, "-20", 10.0, dim_if(pal::ORANGE.gamma_multiply(0.8), enabled));
    title(p, c - vec2(0.0, ROW1_TITLE), "NOISEGATE", enabled);
    let label = if active { format!("{}  ·  -{}", state.format_param(thr), state.format_param(att).replace(" (Off)", "")) } else { "OFF".to_string() };
    mono(p, c + vec2(0.0, ro + 14.0), Align2::CENTER_TOP, &label, 11.0, dim_if(if active { pal::TEXT } else { pal::DIM }, enabled));
}

fn segmented(ui: &mut Ui, state: &PluginContext<Pye4060Params>, pid: u32, rect: Rect, labels: &[&str]) {
    let n = labels.len();
    let v = state.get_param(pid);
    let active = (v * (n - 1) as f32).round() as usize;
    let w = rect.width() / n as f32;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + w * i as f32, rect.top()), vec2(w, rect.height()));
        let resp = ui.interact(r, ui.id().with(("pye-seg", pid, i)), Sense::click());
        if resp.clicked() {
            set_now(state, pid, i as f32 / (n - 1) as f32);
        }
        let p = ui.painter();
        let on = i == active;
        let fill = if on {
            pal::ORANGE
        } else if resp.hovered() {
            Color32::from_rgb(0x33, 0x33, 0x33)
        } else {
            Color32::from_rgb(0x26, 0x26, 0x26)
        };
        p.rect_filled(r.shrink(1.0), CornerRadius::same(3), fill);
        text(p, r.center(), Align2::CENTER_CENTER, l, 11.0, if on { Color32::from_rgb(0x1a, 0x12, 0x08) } else { pal::DIM });
    }
}

fn toggle_button(ui: &mut Ui, state: &PluginContext<Pye4060Params>, pid: u32, rect: Rect, label: &str, on_color: Color32) -> bool {
    let resp = ui.interact(rect, ui.id().with(("pye-tog", pid)), Sense::click());
    let on = state.get_param(pid) > 0.5;
    if resp.clicked() {
        set_now(state, pid, if on { 0.0 } else { 1.0 });
    }
    let p = ui.painter();
    let fill = if on { on_color } else if resp.hovered() { Color32::from_rgb(0x33, 0x33, 0x33) } else { Color32::from_rgb(0x26, 0x26, 0x26) };
    p.rect_filled(rect, CornerRadius::same(3), fill);
    text(p, rect.center(), Align2::CENTER_CENTER, label, 11.0, if on { Color32::from_rgb(0x1a, 0x12, 0x08) } else { pal::DIM });
    on
}

/// The bat-handle LINK toggle between the two modules on the dual frame.
fn link_switch(ui: &mut Ui, state: &PluginContext<Pye4060Params>, c: Pos2, enabled: bool) {
    let pid = id(P::Link);
    let rect = Rect::from_center_size(c, vec2(44.0, 64.0));
    let resp = ui.interact(rect, ui.id().with("pye-link"), if enabled { Sense::click() } else { Sense::hover() });
    let on = state.get_param(pid) > 0.5;
    if enabled && resp.clicked() {
        set_now(state, pid, if on { 0.0 } else { 1.0 });
    }
    let p = ui.painter();
    let plate = Rect::from_center_size(c, vec2(26.0, 46.0));
    p.rect_filled(plate, CornerRadius::same(13), pal::FACE);
    p.rect_stroke(plate, CornerRadius::same(13), sk(1.0, pal::PANEL_EDGE), StrokeKind::Inside);
    let knob_c = c + vec2(0.0, if on { -11.0 } else { 11.0 });
    let col = if on && enabled { pal::ORANGE } else { Color32::from_rgb(0x6a, 0x6a, 0x6a) };
    p.circle_filled(knob_c, 9.0, col);
    text(p, c + vec2(0.0, -36.0), Align2::CENTER_BOTTOM, "LINK", 10.5, dim_if(pal::DIM, enabled));
    mono(p, c + vec2(0.0, 32.0), Align2::CENTER_TOP, if !enabled { "STEREO" } else if on { "C23 A+B" } else { "INDEP." }, 10.0, dim_if(pal::TEXT, enabled));
}

// ---------------------------------------------------------------------------
// Meters

/// Meter deflection for an attenuation in dB. The meter amplifier saturates
/// on the control voltage V = 10^(GR/20) - 1, which compresses the scale at
/// both ends exactly like the printed Pye dial (0-4 close, 8-16 open, 24
/// near the stop).
fn meter_deflection(gr_db: f32) -> f32 {
    let v = 10f32.powf(gr_db.max(0.0) / 20.0) - 1.0;
    (1.14 * v / (v + 3.0)).clamp(0.0, 1.03)
}

#[derive(Clone, Copy, Default)]
struct Needle {
    pos: f32,
    vel: f32,
    peak: f32,
    peak_hold: f32,
}

impl Needle {
    /// Moving-coil ballistics: second order, ~3 Hz, slightly under-damped.
    fn update(&mut self, target: f32, dt: f32) {
        let wn = std::f32::consts::TAU * 3.2;
        let zeta = 0.72;
        let steps = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let acc = wn * wn * (target - self.pos) - 2.0 * zeta * wn * self.vel;
            self.vel += acc * h;
            self.pos += self.vel * h;
        }
        self.pos = self.pos.clamp(-0.02, 1.05);
        if self.pos >= self.peak {
            self.peak = self.pos;
            self.peak_hold = 1.2;
        } else if self.peak_hold > 0.0 {
            self.peak_hold -= dt;
        } else {
            self.peak = (self.peak - dt * 0.35).max(self.pos);
        }
    }
}

fn paint_gr_meter(p: &Painter, rect: Rect, n: &Needle, gr_db: f32) {
    p.rect_filled(rect, CornerRadius::same(6), pal::FACE);
    p.rect_stroke(rect, CornerRadius::same(6), sk(1.0, pal::PANEL_EDGE), StrokeKind::Inside);
    let p = p.with_clip_rect(rect.shrink(1.0));
    let pivot = pos2(rect.center().x, rect.bottom() + rect.height() * 0.62);
    let r_arc = pivot.y - (rect.top() + 42.0);
    let span = 0.62f32;
    let ang = |d: f32| -std::f32::consts::FRAC_PI_2 - span + 2.0 * span * d;

    // Scale.
    arc(&p, pivot, r_arc, ang(0.0), ang(1.0), sk(1.2, pal::FAINT));
    arc(&p, pivot, r_arc + 2.5, ang(meter_deflection(12.0)), ang(1.0), sk(2.5, pal::ORANGE.gamma_multiply(0.55)));
    for gr in (0..=24).step_by(2) {
        let a = ang(meter_deflection(gr as f32));
        let major = gr % 4 == 0;
        let (r0, r1) = if major { (r_arc - 9.0, r_arc + 4.0) } else { (r_arc - 5.0, r_arc + 1.0) };
        p.line_segment([pivot + dir(a) * r0, pivot + dir(a) * r1], sk(if major { 1.6 } else { 1.0 }, if major { pal::TEXT } else { pal::FAINT }));
        if major {
            text(&p, pivot + dir(a) * (r_arc + 14.0), Align2::CENTER_CENTER, &gr.to_string(), 11.0, pal::TEXT);
        }
    }
    text(&p, pos2(rect.center().x, rect.bottom() - 46.0), Align2::CENTER_CENTER, "dB", 14.0, pal::DIM);
    text(&p, pos2(rect.left() + 10.0, rect.bottom() - 10.0), Align2::LEFT_BOTTOM, "ATTENUATION", 9.5, pal::FAINT);
    mono(&p, pos2(rect.right() - 10.0, rect.bottom() - 9.0), Align2::RIGHT_BOTTOM, &format!("{:4.1} dB", gr_db.max(0.0)), 12.0, pal::ORANGE);

    // Peak marker and needle.
    if n.peak > 0.01 {
        let a = ang(n.peak.min(1.03));
        let tip = pivot + dir(a) * (r_arc - 11.0);
        p.circle_filled(tip, 2.2, pal::ORANGE);
    }
    let a = ang(n.pos);
    let base = pivot + dir(a) * (r_arc * 0.42);
    let tip = pivot + dir(a) * (r_arc + 6.0);
    p.line_segment([base, tip], sk(1.8, pal::WARM));
    p.line_segment([pivot + dir(a) * (r_arc - 16.0), tip], sk(2.2, pal::ORANGE));
}

/// Output level bar: VU-ballistic level with sample-peak tick, dBFS scale.
fn paint_out_bar(p: &Painter, rect: Rect, vu_db: f32, peak_db: f32, clip: bool) {
    p.rect_filled(rect, CornerRadius::same(3), pal::FACE);
    let lo = -48.0;
    let y_of = |db: f32| rect.bottom() - rect.height() * ((db - lo) / -lo).clamp(0.0, 1.0);
    let inner = rect.shrink2(vec2(2.0, 2.0));
    if vu_db > lo {
        let top = y_of(vu_db);
        // Green to -18 dBFS (0 VU), yellow to -6, red above.
        for (from, to, col) in [(lo, -18.0, pal::GREEN), (-18.0, -6.0, pal::YELLOW), (-6.0, 0.0, pal::RED)] {
            let y0 = y_of(from);
            let y1 = y_of(to).max(top);
            if y1 < y0 {
                p.rect_filled(Rect::from_x_y_ranges(inner.x_range(), y1..=y0), CornerRadius::ZERO, col);
            }
        }
    }
    if peak_db > lo {
        let y = y_of(peak_db);
        p.line_segment([pos2(inner.left(), y), pos2(inner.right(), y)], sk(1.5, pal::WARM));
    }
    let led = Rect::from_min_size(pos2(rect.left(), rect.top() - 9.0), vec2(rect.width(), 5.0));
    p.rect_filled(led, CornerRadius::same(1), if clip { pal::RED } else { Color32::from_rgb(0x3a, 0x1c, 0x1a) });
    for db in [-36.0, -24.0, -18.0, -12.0, -6.0] {
        let y = y_of(db);
        p.line_segment([pos2(rect.left() - 4.0, y), pos2(rect.left() - 1.0, y)], sk(1.0, pal::FAINT));
    }
}

// ---------------------------------------------------------------------------
// Layout

struct PyeUi {
    needles: [Needle; 2],
    clip_hold: [f32; 2],
    peak_hold: [(f32, f32); 2],
}

impl PyeUi {
    #[allow(clippy::too_many_lines)]
    fn channel(&mut self, ui: &mut Ui, state: &PluginContext<Pye4060Params>, ch: usize, x0: f32, y0: f32, mode: usize, dt: f32) {
        let pw = 370.0;
        let ph = 476.0;
        let panel = Rect::from_min_size(pos2(x0, y0), vec2(pw, ph));
        let enabled = !(ch == 1 && mode == 1);
        {
            let p = ui.painter();
            p.rect_filled(panel, CornerRadius::same(8), pal::PANEL);
            p.rect_stroke(panel, CornerRadius::same(8), sk(1.0, pal::PANEL_EDGE), StrokeKind::Inside);
            let badge = Rect::from_min_size(pos2(x0 + 14.0, y0 + 10.0), vec2(20.0, 18.0));
            p.rect_filled(badge, CornerRadius::same(3), if enabled { pal::ORANGE } else { pal::FAINT });
            text(p, badge.center(), Align2::CENTER_CENTER, if ch == 0 { "A" } else { "B" }, 12.0, Color32::from_rgb(0x1a, 0x12, 0x08));
            let role = match (mode, ch) {
                (0, 0) => "LEFT",
                (0, _) => "RIGHT",
                (1, 0) => "STEREO  ·  L + R",
                (1, _) => "FOLLOWS A",
                (_, 0) => "MID",
                _ => "SIDE",
            };
            text(p, pos2(x0 + 42.0, y0 + 19.0), Align2::LEFT_CENTER, role, 11.5, pal::TEXT);
        }

        // Meter + output bar.
        let (gr_id, out_id, peak_id) = if ch == 0 { (P::GrMeterA, P::OutMeterA, P::PeakMeterA) } else { (P::GrMeterB, P::OutMeterB, P::PeakMeterB) };
        let gr = state.get_meter(id(gr_id));
        let gr = if gr.is_finite() { gr } else { 0.0 };
        self.needles[ch].update(meter_deflection(gr), dt);
        let to_db = |v: f32| if v.is_finite() && v > 1e-6 { 20.0 * v.log10() } else { -120.0 };
        let peak = to_db(state.get_meter(id(peak_id)));
        let (held, timer) = &mut self.peak_hold[ch];
        if peak >= *held {
            *held = peak;
            *timer = 1.0;
        } else if *timer > 0.0 {
            *timer -= dt;
        } else {
            *held = (*held - dt * 20.0).max(peak);
        }
        if peak > -0.1 {
            self.clip_hold[ch] = 1.5;
        } else {
            self.clip_hold[ch] = (self.clip_hold[ch] - dt).max(0.0);
        }
        let meter = Rect::from_min_size(pos2(x0 + 14.0, y0 + 36.0), vec2(316.0, 142.0));
        let bar = Rect::from_min_size(pos2(x0 + 342.0, y0 + 46.0), vec2(12.0, 132.0));
        {
            let p = ui.painter();
            paint_gr_meter(p, meter, &self.needles[ch], gr.max(0.0));
            paint_out_bar(p, bar, to_db(state.get_meter(id(out_id))), *held, self.clip_hold[ch] > 0.0);
        }

        let ids = if ch == 0 {
            (P::GateThreshA, P::GateAttenA, P::ThresholdA, P::DecayA, P::RatioA, P::OutputA)
        } else {
            (P::GateThreshB, P::GateAttenB, P::ThresholdB, P::DecayB, P::RatioB, P::OutputB)
        };
        let row1 = y0 + 276.0;
        let row2 = y0 + 420.0;
        gate_control(ui, state, id(ids.0), id(ids.1), pos2(x0 + 100.0, row1), enabled);
        knob(ui, state, id(ids.2), pos2(x0 + 270.0, row1), 29.0, "THRESHOLD", pal::WARM, false, Some(20), enabled, Some(ROW1_TITLE));
        {
            // Dial legend like the hardware's "- 4 0 4 +" ring.
            let p = ui.painter();
            let c = pos2(x0 + 270.0, row1);
            for (db, l) in [(-24.0f32, "-24"), (-8.0, "-8"), (0.0, "0"), (8.0, "+8"), (16.0, "+16")] {
                let a = START + SWEEP * (db + 24.0) / 40.0;
                text(p, c + dir(a) * 52.0, Align2::CENTER_CENTER, l, 9.5, dim_if(pal::FAINT.gamma_multiply(1.6), enabled));
            }
            for i in 0..=20 {
                let a = START + SWEEP * i as f32 / 20.0;
                p.line_segment([c + dir(a) * 38.5, c + dir(a) * 41.0], sk(1.0, dim_if(pal::FAINT, enabled)));
            }
        }
        stepped(ui, state, id(ids.3), pos2(x0 + 70.0, row2), 22.0, "DECAY  × 100 ms", &["1", "2", "4", "8", "16", "32"], enabled);
        stepped(ui, state, id(ids.4), pos2(x0 + 185.0, row2), 22.0, "RATIO", &["1:1", "2:1", "3:1", "5:1", "LIM"], enabled);
        knob(ui, state, id(ids.5), pos2(x0 + 300.0, row2), 22.0, "OUTPUT", pal::WARM, true, None, enabled, Some(52.0));

        if !enabled {
            let p = ui.painter();
            let veil = Rect::from_min_max(pos2(x0 + 1.0, y0 + 190.0), pos2(x0 + pw - 1.0, y0 + ph - 1.0));
            p.rect_filled(veil, CornerRadius::same(8), Color32::from_rgba_unmultiplied(0x25, 0x25, 0x25, 215));
            let pill = Rect::from_center_size(veil.center(), vec2(270.0, 30.0));
            p.rect_filled(pill, CornerRadius::same(15), pal::FACE);
            p.rect_stroke(pill, CornerRadius::same(15), sk(1.0, pal::PANEL_EDGE), StrokeKind::Inside);
            text(p, pill.center(), Align2::CENTER_CENTER, "STEREO  ·  CHANNEL A CONTROLS BOTH", 11.0, pal::DIM);
        }
    }

    fn master(&mut self, ui: &mut Ui, state: &PluginContext<Pye4060Params>, x0: f32, y0: f32, mode: usize) {
        let w = 136.0;
        let panel = Rect::from_min_size(pos2(x0, y0), vec2(w, 476.0));
        {
            let p = ui.painter();
            p.rect_filled(panel, CornerRadius::same(8), Color32::from_rgb(0x21, 0x21, 0x21));
            p.rect_stroke(panel, CornerRadius::same(8), sk(1.0, pal::PANEL_EDGE), StrokeKind::Inside);
            text(p, pos2(x0 + w / 2.0, y0 + 19.0), Align2::CENTER_CENTER, "MASTER", 11.5, pal::TEXT);
        }
        let cx = x0 + w / 2.0;
        link_switch(ui, state, pos2(cx, y0 + 110.0), mode != 1);
        knob(ui, state, id(P::Mix), pos2(cx, y0 + 250.0), 24.0, "MIX", pal::ORANGE, false, None, true, None);
        stepped(ui, state, id(P::ScFilter), pos2(cx, y0 + 392.0), 20.0, "SIDECHAIN HPF", &["OFF", "60", "90", "150"], true);
    }
}

impl EditorUi<Pye4060Params> for PyeUi {
    fn ui(&mut self, ui: &mut Ui, state: &PluginContext<Pye4060Params>) {
        ui.ctx().request_repaint();
        let dt = ui.input(|i| i.stable_dt).clamp(0.001, 0.05);
        let full = Rect::from_min_size(pos2(0.0, 0.0), vec2(W, H));
        ui.painter().rect_filled(full, CornerRadius::ZERO, pal::BG);

        // Header.
        {
            let p = ui.painter();
            p.rect_filled(Rect::from_min_size(Pos2::ZERO, vec2(W, 48.0)), CornerRadius::ZERO, pal::HEADER);
            let lc = pos2(32.0, 24.0);
            p.circle_stroke(lc, 15.0, sk(2.0, pal::WARM));
            text(p, lc, Align2::CENTER_CENTER, "PYE", 10.5, pal::WARM);
            text(p, pos2(56.0, 24.0), Align2::LEFT_CENTER, "4060", 22.0, pal::TEXT);
            text(p, pos2(112.0, 18.0), Align2::LEFT_CENTER, "COMPRESSION AMPLIFIER", 10.5, pal::DIM);
            text(p, pos2(112.0, 31.0), Align2::LEFT_CENTER, "PWM COMPRESSOR / LIMITER  ·  845751/01", 9.5, pal::FAINT);
        }
        let mode = (state.get_param(id(P::Mode)) * 2.0).round() as usize;
        segmented(ui, state, id(P::Mode), Rect::from_min_size(pos2(392.0, 12.0), vec2(220.0, 24.0)), &["DUAL MONO", "STEREO", "M/S"]);
        {
            let p = ui.painter();
            text(p, pos2(632.0, 24.0), Align2::LEFT_CENTER, "ANALOG", 10.5, pal::DIM);
        }
        segmented(ui, state, id(P::Analog), Rect::from_min_size(pos2(682.0, 12.0), vec2(132.0, 24.0)), &["OFF", "50 Hz", "60 Hz"]);
        toggle_button(ui, state, id(P::Bypass), Rect::from_min_size(pos2(826.0, 12.0), vec2(82.0, 24.0)), "BYPASS", pal::YELLOW);

        self.channel(ui, state, 0, 12.0, 56.0, mode, dt);
        self.master(ui, state, 392.0, 56.0, mode);
        self.channel(ui, state, 1, 538.0, 56.0, mode, dt);

        let p = ui.painter();
        text(p, pos2(14.0, H - 14.0), Align2::LEFT_CENTER, "JARDIM SONORO", 10.0, pal::FAINT);
        text(p, pos2(W - 14.0, H - 14.0), Align2::RIGHT_CENTER, "drag to adjust  ·  shift: fine / free threshold  ·  double-click: default  ·  click a scale step to select", 10.0, pal::FAINT);
    }
}

pub fn create(params: Arc<Pye4060Params>) -> Box<dyn Editor> {
    let ui = PyeUi { needles: [Needle::default(); 2], clip_hold: [0.0; 2], peak_hold: [(-120.0, 0.0); 2] };
    let mut visuals = truce_egui::theme::dark();
    visuals.panel_fill = pal::BG;
    Box::new(EguiEditor::with_ui(params, (W as u32, H as u32), ui).with_visuals(visuals).resizable(false))
}
