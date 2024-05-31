use crate::{
    color::ColorU,

    fonts::{FamilyId, Properties},
    geometry::vector::{vec2f, Vector2F},

    AfterLayoutContext,
    AppContext,
    Element,
    Event,
    EventContext,
    LayoutContext,
    MutableAppContext,
    PaintContext,
    SizeConstraint
};

use std::{ops::Range, sync::Arc};

pub struct Label {
    text: String,
    family_id: FamilyId,
    font_properties: Properties,
    font_size: f32,
    highlights: Option<Highlights>,
    layout_line: Option<Arc<Line>>,
    colors: Option<Vec<(Range<usize>, ColorU)>>,
    size: Option<Vector2F>
}

pub struct Highlights {
    color: ColorU,
    indices: Vec<usize>,
    font_properties: Properties
}

impl Label {
    pub fn new(text: String, family_id: FamilyId, font_size: f32) -> Self {
        Self {
            text,
            family_id,
            font_properties: Properties::new(),
            font_size,

            highlights: None,
            layout_line: None,
            colors: None,
            size: None
        }
    }

    pub fn with_highlights(
        mut self,

        color: ColorU,
        font_properties: Properties,
        indices: Vec<usize>
    ) -> Self {
        self.highlights = Some(Highlights {
            color,
            font_properties,
            indices
        });

        self
    }
}

impl Element for Label {
    fn layout(
        &mut self,

        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        _: &AppContext
    ) -> Vector2F {
        let font_id = ctx
            .font_cache
            .select_font(self.family_id, &self.font_properties)
            .unwrap();

        let text_len = self.text.chars().count();

        let mut styles;
        let mut colors;

        if let Some(highlights) = self.highlights.as_ref() {
            styles = Vec::new();
            colors = Vec::new();

            let highlight_font_id = ctx
                .font_cache
                .select_font(self.family_id, &highlights.font_properties)
                .unwrap_or(font_id);

            let mut pending_highlight: Option<Range<usize>> = None;

            for ix in &highlights.indices {
                if let Some(pending_highlight) = pending_highlight.as_mut() {
                    if *ix == pending_highlight.end {
                        pending_highlight.end += 1;
                    } else {
                        styles.push((pending_highlight.clone(), highlight_font_id));
                        colors.push((pending_highlight.clone(), highlights.color));

                        styles.push((pending_highlight.end..*ix, font_id));
                        colors.push((pending_highlight.end..*ix, ColorU::black()));

                        *pending_highlight = *ix..*ix + 1;
                    }
                } else {
                    styles.push((0..*ix, font_id));
                    colors.push((0..*ix, ColorU::black()));

                    pending_highlight = Some(*ix..*ix + 1);
                }
            }

            if let Some(pending_highlight) = pending_highlight.as_mut() {
                styles.push((pending_highlight.clone(), highlight_font_id));
                colors.push((pending_highlight.clone(), highlights.color));

                if text_len > pending_highlight.end {
                    styles.push((pending_highlight.end..text_len, font_id));

                    colors.push((pending_highlight.end..text_len, ColorU::black()));
                }
            } else {
                styles.push((0..text_len, font_id));

                colors.push((0..text_len, ColorU::black()));
            }
        } else {
            styles = vec![(0..text_len, font_id)];
            colors = vec![(0..text_len, ColorU::black())];
        }

        self.colors = Some(colors);

        let layout_line = ctx.text_layout_cache.layout_str(
            self.text.as_str(),
            self.font_size,

            styles.as_slice(),
            ctx.font_cache
        );

        let size = vec2f(
            layout_line
                .width
                .max(constraint.min.x())
                .min(constraint.max.x()),

            ctx.font_cache.line_height(font_id, self.font_size).ceil()
        );

        self.layout_line = Some(layout_line);
        self.size = Some(size);

        size
    }

    fn after_layout(&mut self, _: &mut AfterLayoutContext, _: &mut MutableAppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _: &AppContext) {
        // ctx.canvas.set_fill_style(FillStyle::Color(ColorU::black()));
        //
        // self.layout_line.as_ref().unwrap().paint(
        //     origin,
        //
        //     RectF::new(origin, self.size.unwrap()),
        //     self.colors.as_ref().unwrap(),
        //
        //     ctx.canvas,
        //     ctx.font_cache
        // );
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn dispatch_event(&self, _: &Event, _: &mut EventContext, _: &AppContext) -> bool {
        false
    }
}