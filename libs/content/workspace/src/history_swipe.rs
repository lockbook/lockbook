use egui::emath::easing;
use egui::{
    Align2, Id, LayerId, MouseWheelSource, MouseWheelUnit, Order, Rect, TouchPhase, Vec2, pos2,
};

use crate::style::chrome::{overlay_shadow, phosphor, phosphor_font_id};
use crate::tab::{ContentState, TabContent};
use crate::theme::palette_v2::ThemeExt as _;
use crate::workspace::Workspace;

const THRESHOLD: f32 = 96.0;
const RADIUS: f32 = 20.0;
const INSET: f32 = 16.0;
const OVERSHOOT: f32 = 0.15;
const OVERSHOOT_MAX: f32 = 16.0;
const GLYPH: f32 = 20.0;
const SETTLE_SECS: f32 = 0.18;
const ARM_SECS: f32 = 0.1;
const SLOP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwipeDir {
    Back,
    Forward,
}

#[derive(Default)]
pub struct HistorySwipe {
    pull: Option<Pull>,
    settle: Option<Settle>,
    wheel: Wheel,
}

#[derive(Clone, Copy)]
struct Pull {
    dir: SwipeDir,
    travel: f32,
    y: Option<f32>,
}

#[derive(Clone, Copy)]
struct Settle {
    pull: Pull,
    went: bool,
    since: f64,
}

#[derive(Default, Clone, Copy)]
enum Wheel {
    #[default]
    Idle,
    Deciding {
        travel: Vec2,
        content: bool,
    },
    Swiping,
    Content,
}

struct WheelEvent {
    unit: MouseWheelUnit,
    delta: Vec2,
    phase: TouchPhase,
    source: MouseWheelSource,
    zoom: bool,
}

impl HistorySwipe {
    fn pull(&mut self, dir: SwipeDir, travel: f32, y: Option<f32>) -> bool {
        let travel = travel.max(0.0);
        self.pull = Some(Pull { dir, travel, y });
        self.settle = None;
        travel >= THRESHOLD
    }

    fn release(&mut self, now: f64, can: impl Fn(SwipeDir) -> bool) -> Option<SwipeDir> {
        let pull = self.pull.take()?;
        let went = pull.travel >= THRESHOLD && can(pull.dir);
        self.settle = Some(Settle { pull, went, since: now });
        went.then_some(pull.dir)
    }

    fn cancel(&mut self, now: f64) {
        if let Some(pull) = self.pull.take() {
            self.settle = Some(Settle { pull, went: false, since: now });
        }
    }

    fn wheel(
        &mut self, events: &[WheelEvent], hovered: bool, scrolling: bool, now: f64,
        can: impl Fn(SwipeDir) -> bool,
    ) -> Option<SwipeDir> {
        let mut went = None;
        for e in events {
            if e.source == MouseWheelSource::Momentum {
                continue;
            }
            match e.phase {
                TouchPhase::Start => {
                    went = went.or(self.end_wheel(now, &can));
                    self.wheel = Self::wheel_start(hovered);
                }
                TouchPhase::End | TouchPhase::Cancel => went = went.or(self.end_wheel(now, &can)),
                TouchPhase::Move => {
                    if matches!(self.wheel, Wheel::Idle) {
                        self.wheel = Self::wheel_start(hovered);
                    }
                    let fingers = e.source == MouseWheelSource::Trackpad
                        || (e.source == MouseWheelSource::Unknown
                            && e.unit == MouseWheelUnit::Point);
                    if !fingers || e.zoom {
                        if !matches!(self.wheel, Wheel::Swiping) {
                            self.wheel = Wheel::Content;
                        }
                        continue;
                    }
                    self.wheel_moved(e.delta, &can);
                }
            }
        }
        if events.is_empty() && !scrolling {
            went = went.or(self.end_wheel(now, &can));
        }
        went
    }

    fn wheel_start(hovered: bool) -> Wheel {
        if hovered {
            Wheel::Deciding { travel: Vec2::ZERO, content: false }
        } else {
            Wheel::Content
        }
    }

    fn wheel_moved(&mut self, delta: Vec2, can: impl Fn(SwipeDir) -> bool) {
        match self.wheel {
            Wheel::Deciding { travel, content } => {
                let travel = travel + delta;
                if travel.length() < SLOP {
                    self.wheel = Wheel::Deciding { travel, content };
                    return;
                }
                let dir = if travel.x > 0.0 { SwipeDir::Back } else { SwipeDir::Forward };
                let sideways = travel.x.abs() > 2.0 * travel.y.abs();
                if !content && sideways && can(dir) {
                    self.wheel = Wheel::Swiping;
                    self.pull(dir, travel.x.abs(), None);
                } else {
                    self.wheel = Wheel::Content;
                }
            }
            Wheel::Swiping => {
                if let Some(pull) = self.pull {
                    let along = match pull.dir {
                        SwipeDir::Back => delta.x,
                        SwipeDir::Forward => -delta.x,
                    };
                    self.pull(pull.dir, pull.travel + along, pull.y);
                }
            }
            Wheel::Idle | Wheel::Content => {}
        }
    }

    fn end_wheel(&mut self, now: f64, can: impl Fn(SwipeDir) -> bool) -> Option<SwipeDir> {
        let swiping = matches!(self.wheel, Wheel::Swiping);
        self.wheel = Wheel::Idle;
        if swiping { self.release(now, can) } else { None }
    }

    fn content_scrolled(&mut self) {
        if let Wheel::Deciding { content, .. } = &mut self.wheel {
            *content = true;
        }
    }
}

impl Workspace {
    pub fn history_swipe(&mut self, dir: SwipeDir, travel: f32, y: Option<f32>) -> bool {
        self.ctx.request_repaint();
        self.swipe.pull(dir, travel, y)
    }

    pub fn history_swipe_end(&mut self) -> bool {
        let now = self.ctx.input(|i| i.time);
        let (back, forward) = (self.can_back(), self.can_forward());
        let went = self.swipe.release(now, |dir| match dir {
            SwipeDir::Back => back,
            SwipeDir::Forward => forward,
        });
        self.go_swiped(went)
    }

    pub fn history_swipe_cancel(&mut self) {
        let now = self.ctx.input(|i| i.time);
        self.swipe.cancel(now);
        self.ctx.request_repaint();
    }

    fn go_swiped(&mut self, went: Option<SwipeDir>) -> bool {
        self.ctx.request_repaint();
        match went {
            Some(SwipeDir::Back) => self.back(),
            Some(SwipeDir::Forward) => self.forward(),
            None => return false,
        }
        true
    }

    fn content_pans_sideways(&self) -> bool {
        match self.current_tab().map(|tab| &tab.content) {
            Some(ContentState::Open(TabContent::Svg(_) | TabContent::Image(_))) => true,
            #[cfg(not(target_family = "wasm"))]
            Some(ContentState::Open(TabContent::MindMap(_))) => true,
            _ => false,
        }
    }

    pub(crate) fn history_swipe_wheel(&mut self, rect: Rect) -> f32 {
        let (events, hovered, scrolling, now) = self.ctx.input(|i| {
            let events: Vec<WheelEvent> = i
                .events
                .iter()
                .filter_map(|e| match *e {
                    egui::Event::MouseWheel { unit, delta, phase, source, modifiers } => {
                        Some(WheelEvent {
                            unit,
                            delta,
                            phase,
                            source,
                            zoom: modifiers.ctrl || modifiers.command,
                        })
                    }
                    _ => None,
                })
                .collect();
            let hovered = i.pointer.hover_pos().is_some_and(|p| rect.contains(p));
            (events, hovered, i.is_scrolling(), i.time)
        });
        let (events, scrolling) =
            if self.trackpad_history_swipe { (events, scrolling) } else { (vec![], false) };

        let sideways = !self.content_pans_sideways();
        let (back, forward) = (self.can_back(), self.can_forward());
        let went = self.swipe.wheel(&events, hovered, scrolling, now, |dir| {
            sideways
                && match dir {
                    SwipeDir::Back => back,
                    SwipeDir::Forward => forward,
                }
        });
        self.go_swiped(went);

        if matches!(self.swipe.wheel, Wheel::Swiping) {
            self.ctx.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
        }
        self.ctx.input(|i| i.smooth_scroll_delta.x)
    }

    pub(crate) fn history_swipe_content_scrolled(&mut self, before: f32) {
        if before != 0.0 && self.ctx.input(|i| i.smooth_scroll_delta.x) == 0.0 {
            self.swipe.content_scrolled();
        }
    }

    pub(crate) fn paint_history_swipe(&mut self, rect: Rect) {
        let now = self.ctx.input(|i| i.time);
        let (pull, fade, armed) = if let Some(pull) = self.swipe.pull {
            (pull, 1.0, pull.travel >= THRESHOLD)
        } else if let Some(settle) = self.swipe.settle {
            let t = ((now - settle.since) as f32 / SETTLE_SECS).clamp(0.0, 1.0);
            if t >= 1.0 {
                self.swipe.settle = None;
                return;
            }
            self.ctx.request_repaint();
            let left = 1.0 - easing::cubic_out(t);
            let mut pull = settle.pull;
            if !settle.went {
                pull.travel *= left;
            }
            (pull, left, settle.went)
        } else {
            return;
        };

        let t = self.ctx.get_lb_theme();
        let armed =
            self.ctx
                .animate_bool_with_time(Id::unique("history_swipe_armed"), armed, ARM_SECS);

        let progress = (pull.travel / THRESHOLD).clamp(0.0, 1.0);
        let over = ((pull.travel - THRESHOLD).max(0.0) * OVERSHOOT).min(OVERSHOOT_MAX);
        let inward = (2.0 * RADIUS + INSET) * progress + over - RADIUS;
        let x = match pull.dir {
            SwipeDir::Back => rect.left() + inward,
            SwipeDir::Forward => rect.right() - inward,
        };
        let margin = 3.0 * RADIUS;
        let y = pull.y.unwrap_or(rect.center().y);
        let y = if rect.height() > 2.0 * margin {
            y.clamp(rect.top() + margin, rect.bottom() - margin)
        } else {
            rect.center().y
        };

        let alpha = (progress * 1.5).min(1.0) * fade;
        let radius = RADIUS * (1.0 + 0.08 * armed);
        let bubble = Rect::from_center_size(pos2(x, y), Vec2::splat(2.0 * radius));
        let fill = t.neutral_bg().lerp_to_gamma(t.accent(), armed);
        let ink = t
            .neutral_fg_secondary()
            .lerp_to_gamma(t.neutral_bg(), armed);
        let mut shadow = overlay_shadow(&t);
        shadow.color = shadow.color.gamma_multiply(alpha);
        let glyph = match pull.dir {
            SwipeDir::Back => phosphor::ARROW_LEFT,
            SwipeDir::Forward => phosphor::ARROW_RIGHT,
        };

        let painter = self
            .ctx
            .layer_painter(LayerId::new(Order::Foreground, Id::unique("history_swipe")))
            .with_clip_rect(rect);
        painter.add(shadow.as_shape(bubble, radius));
        painter.rect_filled(bubble, radius, fill.gamma_multiply(alpha));
        painter.text(
            bubble.center(),
            Align2::CENTER_CENTER,
            glyph,
            phosphor_font_id(GLYPH),
            ink.gamma_multiply(alpha),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    fn event(phase: TouchPhase, source: MouseWheelSource, x: f32, y: f32) -> WheelEvent {
        WheelEvent { unit: MouseWheelUnit::Point, delta: vec2(x, y), phase, source, zoom: false }
    }

    fn fingers(phase: TouchPhase, x: f32, y: f32) -> WheelEvent {
        event(phase, MouseWheelSource::Trackpad, x, y)
    }

    fn both(_: SwipeDir) -> bool {
        true
    }

    fn frame(s: &mut HistorySwipe, e: WheelEvent) -> Option<SwipeDir> {
        s.wheel(&[e], true, true, 0.0, both)
    }

    fn gesture(s: &mut HistorySwipe, moves: &[(f32, f32)]) -> Option<SwipeDir> {
        let mut went = frame(s, fingers(TouchPhase::Start, 0.0, 0.0));
        for &(x, y) in moves {
            went = went.or(frame(s, fingers(TouchPhase::Move, x, y)));
        }
        went.or(frame(s, fingers(TouchPhase::End, 0.0, 0.0)))
    }

    #[test]
    fn swiping_right_past_the_threshold_goes_back() {
        let mut s = HistorySwipe::default();
        assert_eq!(gesture(&mut s, &[(10.0, 1.0); 12]), Some(SwipeDir::Back));
        assert!(s.pull.is_none());
        assert!(s.settle.is_some_and(|settle| settle.went));
    }

    #[test]
    fn swiping_left_past_the_threshold_goes_forward() {
        let mut s = HistorySwipe::default();
        assert_eq!(gesture(&mut s, &[(-10.0, 0.0); 12]), Some(SwipeDir::Forward));
    }

    #[test]
    fn a_short_swipe_shows_the_arrow_and_settles_back() {
        let mut s = HistorySwipe::default();
        frame(&mut s, fingers(TouchPhase::Start, 0.0, 0.0));
        for _ in 0..5 {
            frame(&mut s, fingers(TouchPhase::Move, 10.0, 0.0));
        }
        assert!(
            s.pull
                .is_some_and(|p| p.dir == SwipeDir::Back && p.travel == 50.0)
        );
        assert_eq!(frame(&mut s, fingers(TouchPhase::End, 0.0, 0.0)), None);
        assert!(s.settle.is_some_and(|settle| !settle.went));
    }

    #[test]
    fn turning_back_past_the_start_goes_nowhere() {
        let mut s = HistorySwipe::default();
        let mut moves = vec![(10.0, 0.0); 12];
        moves.extend([(-10.0, 0.0); 20]);
        assert_eq!(gesture(&mut s, &moves), None);
    }

    #[test]
    fn a_vertical_scroll_is_left_to_the_content() {
        let mut s = HistorySwipe::default();
        let mut moves = vec![(1.0, 10.0)];
        moves.extend([(20.0, 0.0); 10]);
        assert_eq!(gesture(&mut s, &moves), None);
        assert!(s.pull.is_none() && s.settle.is_none());
    }

    #[test]
    fn without_history_that_way_the_content_keeps_the_scroll() {
        let mut s = HistorySwipe::default();
        let only_forward = |dir| dir == SwipeDir::Forward;
        s.wheel(&[fingers(TouchPhase::Start, 0.0, 0.0)], true, true, 0.0, only_forward);
        for _ in 0..12 {
            s.wheel(&[fingers(TouchPhase::Move, 10.0, 0.0)], true, true, 0.0, only_forward);
        }
        assert!(s.pull.is_none());
        assert!(matches!(s.wheel, Wheel::Content));
    }

    #[test]
    fn content_that_scrolled_sideways_keeps_the_gesture() {
        let mut s = HistorySwipe::default();
        frame(&mut s, fingers(TouchPhase::Start, 0.0, 0.0));
        frame(&mut s, fingers(TouchPhase::Move, 4.0, 0.0));
        s.content_scrolled();
        let mut went = None;
        for _ in 0..12 {
            went = went.or(frame(&mut s, fingers(TouchPhase::Move, 10.0, 0.0)));
        }
        assert_eq!(went.or(frame(&mut s, fingers(TouchPhase::End, 0.0, 0.0))), None);
        assert!(s.pull.is_none() && s.settle.is_none());
    }

    #[test]
    fn a_mouse_wheel_never_swipes() {
        let mut s = HistorySwipe::default();
        let mut went = None;
        for _ in 0..12 {
            let e = event(TouchPhase::Move, MouseWheelSource::Wheel, 10.0, 0.0);
            went = went.or(frame(&mut s, e));
        }
        assert_eq!(went.or(s.wheel(&[], true, false, 0.0, both)), None);
        assert!(s.pull.is_none());
    }

    #[test]
    fn a_zoom_never_swipes() {
        let mut s = HistorySwipe::default();
        frame(&mut s, fingers(TouchPhase::Start, 0.0, 0.0));
        for _ in 0..12 {
            frame(&mut s, WheelEvent { zoom: true, ..fingers(TouchPhase::Move, 10.0, 0.0) });
        }
        assert!(s.pull.is_none());
    }

    #[test]
    fn a_scroll_outside_the_workspace_never_swipes() {
        let mut s = HistorySwipe::default();
        s.wheel(&[fingers(TouchPhase::Start, 0.0, 0.0)], false, true, 0.0, both);
        for _ in 0..12 {
            s.wheel(&[fingers(TouchPhase::Move, 10.0, 0.0)], true, true, 0.0, both);
        }
        assert!(s.pull.is_none());
    }

    #[test]
    fn momentum_after_lifting_is_ignored() {
        let mut s = HistorySwipe::default();
        let mut moves = vec![(10.0, 0.0); 5];
        moves.extend([(0.0, 0.0); 1]);
        assert_eq!(gesture(&mut s, &moves), None);
        for _ in 0..12 {
            frame(&mut s, event(TouchPhase::Move, MouseWheelSource::Momentum, 10.0, 0.0));
        }
        assert!(s.pull.is_none());
        assert!(matches!(s.wheel, Wheel::Idle));
    }

    #[test]
    fn without_phases_the_swipe_ends_when_scrolling_stops() {
        let mut s = HistorySwipe::default();
        for _ in 0..12 {
            frame(&mut s, event(TouchPhase::Move, MouseWheelSource::Unknown, 10.0, 0.0));
        }
        assert!(s.pull.is_some());
        assert_eq!(s.wheel(&[], true, true, 0.0, both), None);
        assert!(s.pull.is_some());
        assert_eq!(s.wheel(&[], true, false, 0.0, both), Some(SwipeDir::Back));
    }

    #[test]
    fn a_touch_pull_arms_at_the_threshold() {
        let mut s = HistorySwipe::default();
        assert!(!s.pull(SwipeDir::Back, THRESHOLD - 1.0, Some(300.0)));
        assert!(s.pull(SwipeDir::Back, THRESHOLD, Some(300.0)));
        assert_eq!(s.release(1.0, |dir| dir == SwipeDir::Forward), None);
        assert!(
            s.settle
                .is_some_and(|settle| !settle.went && settle.since == 1.0)
        );
    }
}
