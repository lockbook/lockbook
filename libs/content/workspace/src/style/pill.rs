//! A pill: a few words or a count on a fully round fill, led by a dot or a
//! mark when it has one. Set in [`TypeRole::Mono`], centred on its line box
//! like a button's label.

use egui::{Color32, CornerRadius, Painter, Rect, Response, Sense, Ui, UiBuilder, Widget, vec2};

use super::{Space, TypeRole};
use crate::widgets::GlyphonLabel;

/// What leads a pill's text.
pub enum PillLead<'a> {
    /// A dot in a color: a label's own.
    Dot(Color32),
    /// A mark painted into the square it's given.
    Mark(&'a dyn Fn(&Painter, Rect)),
}

pub struct Pill<'a> {
    spans: Vec<(&'a str, Color32)>,
    fill: Color32,
    lead: Option<PillLead<'a>>,
}

impl<'a> Pill<'a> {
    /// Text in `spans`, each in its own color, on `fill`.
    pub fn new(spans: Vec<(&'a str, Color32)>, fill: Color32) -> Self {
        Self { spans, fill, lead: None }
    }

    pub fn lead(mut self, lead: PillLead<'a>) -> Self {
        self.lead = Some(lead);
        self
    }

    pub fn height() -> f32 {
        TypeRole::Mono.line_height() + Space::Xxs.pts()
    }

    pub fn width(&self, ui: &Ui) -> f32 {
        self.start() + self.label().measure(ui).x + PAD
    }

    /// Where the text starts: past the round end, and past the lead when
    /// there is one, which sits further into the end than text would.
    fn start(&self) -> f32 {
        match self.lead {
            Some(_) => LEAD_PAD + Self::height() * LEAD + Space::Xxs.pts(),
            None => PAD,
        }
    }

    fn label(&self) -> GlyphonLabel<'a> {
        let spans = self.spans.iter().map(|&(s, c)| (s, Some(c))).collect();
        GlyphonLabel::new_colored(spans, Color32::PLACEHOLDER).role(TypeRole::Mono)
    }
}

/// Space at each round end.
const PAD: f32 = Space::Sm.pts();
/// Space before a lead; a dot or mark is round, so it sits into the end.
const LEAD_PAD: f32 = Space::Xs.pts();
/// A lead's side, as a share of the pill's height.
const LEAD: f32 = 0.7;

impl Widget for Pill<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let h = Self::height();
        let (rect, response) = ui.allocate_exact_size(vec2(self.width(ui), h), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same((h / 2.0) as u8), self.fill);
        let lead = Rect::from_center_size(
            rect.left_center() + vec2(LEAD_PAD + h * LEAD / 2.0, 0.0),
            vec2(h * LEAD, h * LEAD),
        );
        match &self.lead {
            Some(PillLead::Dot(color)) => {
                ui.painter().circle_filled(lead.center(), h * 0.18, *color);
            }
            Some(PillLead::Mark(paint)) => paint(ui.painter(), lead),
            None => {}
        }
        let label = self.label();
        let size = label.measure(ui);
        let at = Rect::from_min_size(rect.left_center() + vec2(self.start(), -size.y / 2.0), size);
        ui.new_child(UiBuilder::new().max_rect(at)).add(label);
        response
    }
}
