use super::{BufferView, DisplayPoint, SelectAction};

use gpui::{
    geometry::{
        rect::RectF,
        vector::{vec2f, Vector2F}
    },

    text_layout::{self, TextLayoutCache},

    AfterLayoutContext, AppContext, Element, Event, EventContext, FontCache, LayoutContext,
    MutableAppContext, PaintContext, Scene, SizeConstraint, ViewHandle
};

use std::{
    cmp::{self},
    sync::Arc
};

pub struct BufferElement {
    view: ViewHandle<BufferView>,
    layout: Option<LayoutState>,
    paint: Option<PaintState>
}

impl BufferElement {
    pub fn new(view: ViewHandle<BufferView>) -> Self {
        Self {
            view,

            layout: None,
            paint: None
        }
    }

    fn mouse_down(
        &self,

        position: Vector2F,
        cmd: bool,

        ctx: &mut EventContext,
        app: &AppContext
    ) -> bool {
        let layout = self.layout.as_ref().unwrap();
        let paint = self.paint.as_ref().unwrap();

        if paint.text_rect.contains_point(position) {
            let view = self.view.as_ref(app);
            let position = paint.point_for_position(view, layout, position, ctx.font_cache, app);

            ctx.dispatch_action("buffer:select", SelectAction::Begin { position, add: cmd });

            true
        } else {
            false
        }
    }

    fn mouse_up(&self, _position: Vector2F, ctx: &mut EventContext, app: &AppContext) -> bool {
        if self.view.as_ref(app).is_selecting() {
            ctx.dispatch_action("buffer:select", SelectAction::End);

            true
        } else {
            false
        }
    }

    fn mouse_dragged(&self, position: Vector2F, ctx: &mut EventContext, app: &AppContext) -> bool {
        let view = self.view.as_ref(app);

        let layout = self.layout.as_ref().unwrap();
        let paint = self.paint.as_ref().unwrap();

        if view.is_selecting() {
            let rect = self.paint.as_ref().unwrap().text_rect;
            let mut scroll_delta = Vector2F::zero();

            let vertical_margin = view.line_height(ctx.font_cache).min(rect.height() / 3.0);

            let top = rect.origin_y() + vertical_margin;
            let bottom = rect.lower_left().y() - vertical_margin;

            if position.y() < top {
                scroll_delta.set_y(-scale_vertical_mouse_autoscroll_delta(top - position.y()))
            }

            if position.y() > bottom {
                scroll_delta.set_y(scale_vertical_mouse_autoscroll_delta(position.y() - bottom))
            }

            let horizontal_margin = view.line_height(ctx.font_cache).min(rect.width() / 3.0);

            let left = rect.origin_x() + horizontal_margin;
            let right = rect.upper_right().x() - horizontal_margin;

            if position.x() < left {
                scroll_delta.set_x(-scale_horizontal_mouse_autoscroll_delta(
                    left - position.x()
                ))
            }

            if position.x() > right {
                scroll_delta.set_x(scale_horizontal_mouse_autoscroll_delta(
                    position.x() - right
                ))
            }

            ctx.dispatch_action(
                "buffer:select",

                SelectAction::Update {
                    position: paint.point_for_position(view, layout, position, ctx.font_cache, app),

                    scroll_position: (view.scroll_position() + scroll_delta).clamp(
                        Vector2F::zero(),

                        self.layout.as_ref().unwrap().scroll_max(
                            view,

                            ctx.font_cache,
                            ctx.text_layout_cache,

                            app
                        )
                    )
                }
            );

            true
        } else {
            false
        }
    }

    fn key_down(&self, chars: &str, ctx: &mut EventContext, app: &AppContext) -> bool {
        if self.view.is_focused(app) {
            if chars.is_empty() {
                false
            } else {
                if chars.chars().any(|c| c.is_control()) {
                    false
                } else {
                    ctx.dispatch_action("buffer:insert", chars.to_string());

                    true
                }
            }
        } else {
            false
        }
    }

    fn scroll(
        &self,

        position: Vector2F,
        delta: Vector2F,

        precise: bool,

        ctx: &mut EventContext,
        app: &AppContext
    ) -> bool {
        let paint = self.paint.as_ref().unwrap();

        if !paint.rect.contains_point(position) {
            return false;
        }

        if !precise {
            todo!("ainda precisa lidar com eventos de rolagem não precisos da roda do mouse");
        }

        let view = self.view.as_ref(app);

        let font_cache = &ctx.font_cache;
        let layout_cache = &ctx.text_layout_cache;

        let max_glyph_width = view.em_width(font_cache);
        let line_height = view.line_height(font_cache);

        let x = (view.scroll_position().x() * max_glyph_width - delta.x()) / max_glyph_width;
        let y = (view.scroll_position().y() * line_height - delta.y()) / line_height;

        let scroll_position = vec2f(x, y).clamp(
            Vector2F::zero(),

            self.layout
                .as_ref()
                .unwrap()
                .scroll_max(view, font_cache, layout_cache, app)
        );

        ctx.dispatch_action("buffer:scroll", scroll_position);

        true
    }

    fn paint_gutter(&mut self, rect: RectF, ctx: &mut PaintContext, app: &AppContext) {
        // if let Some(layout) = self.layout.as_ref() {
        //     let view = self.view.as_ref(app);

        //     let scene = &mut ctx.scene;
        //     let font_cache = &ctx.font_cache;

        //     let line_height = view.line_height(font_cache);
        //     let scroll_top = view.scroll_position().y() * line_height;

        //     scene.save();
        //     scene.translate(rect.origin());
        //     scene.set_fill_style(FillStyle::Color(ColorU::white()));

        //     let rect = RectF::new(Vector2F::zero(), rect.size());
        //     let mut rect_path = Path2D::new();

        //     rect_path.rect(rect);

        //     scene.clip_path(rect_path, FillRule::EvenOdd);
        //     scene.fill_rect(rect);

        //     for (ix, line) in layout.line_number_layouts.iter().enumerate() {
        //         let line_origin = vec2f(
        //             rect.width() - line.width - layout.gutter_padding,
        //             ix as f32 * line_height - (scroll_top % line_height)
        //         );

        //         line.paint(
        //             line_origin,
        //             rect,
        //             &[(0..line.len, ColorU::black())],
        //             scene,
        //             font_cache
        //         );
        //     }

        //     scene.restore();
        // }
    }

    fn paint_text(&mut self, rect: RectF, ctx: &mut PaintContext, app: &AppContext) {
        // if let Some(layout) = self.layout.as_ref() {
        //     let scene = &mut ctx.scene;
        //     let font_cache = &ctx.font_cache;

        //     scene.save();
        //     scene.translate(rect.origin());
        //     scene.set_fill_style(FillStyle::Color(ColorU::white()));

        //     let rect = RectF::new(Vector2F::zero(), rect.size());
        //     let mut rect_path = Path2D::new();

        //     rect_path.rect(rect);

        //     scene.clip_path(rect_path, FillRule::EvenOdd);
        //     scene.fill_rect(rect);

        //     let view = self.view.as_ref(app);
        //     let line_height = view.line_height(font_cache);
        //     let descent = view.font_descent(font_cache);

        //     let start_row = view.scroll_position().y() as u32;
        //     let scroll_top = view.scroll_position().y() * line_height;

        //     let end_row = ((scroll_top + rect.height()) / line_height).ceil() as u32 + 1; // adicionar 1 para garantir que as seleções saiam da tela
        //     let max_glyph_width = view.em_width(font_cache);
        //     let scroll_left = view.scroll_position().x() * max_glyph_width;

        //     // desenhar seleções
        //     scene.save();

        //     let corner_radius = 2.5;
        //     let mut cursors = SmallVec::<[Cursor; 32]>::new();

        //     for selection in view.selections_in_range(
        //         DisplayPoint::new(start_row, 0)..DisplayPoint::new(end_row, 0),

        //         app
        //     ) {
        //         if selection.start != selection.end {
        //             let range_start = cmp::min(selection.start, selection.end);
        //             let range_end = cmp::max(selection.start, selection.end);

        //             let row_range = if range_end.column() == 0 {
        //                 cmp::max(range_start.row(), start_row)..cmp::min(range_end.row(), end_row)
        //             } else {
        //                 cmp::max(range_start.row(), start_row)..cmp::min(range_end.row() + 1, end_row)
        //             };

        //             let selection = Selection {
        //                 line_height,

        //                 start_y: row_range.start as f32 * line_height - scroll_top,

        //                 lines: row_range
        //                     .into_iter()
        //                     .map(|row| {
        //                         let line_layout = &layout.line_layouts[(row - start_row) as usize];

        //                         SelectionLine {
        //                             start_x: if row == range_start.row() {
        //                                 line_layout.x_for_index(range_start.column() as usize)
        //                                     - scroll_left
        //                                     - descent
        //                             } else {
        //                                 -scroll_left
        //                             },

        //                             end_x: if row == range_end.row() {
        //                                 line_layout.x_for_index(range_end.column() as usize)
        //                                     - scroll_left
        //                                     - descent
        //                             } else {
        //                                 line_layout.width + corner_radius * 2.0
        //                                     - scroll_left
        //                                     - descent
        //                             }
        //                         }
        //                     }).collect()
        //             };

        //             selection.paint(scene);
        //         }

        //         if view.cursors_visible() {
        //             let cursor_position = selection.end;

        //             if (start_row..end_row).contains(&cursor_position.row()) {
        //                 let cursor_row_layout = &layout.line_layouts[(selection.end.row() - start_row) as usize];

        //                 cursors.push(Cursor {
        //                     x: cursor_row_layout.x_for_index(selection.end.column() as usize)
        //                         - scroll_left
        //                         - descent,

        //                     y: selection.end.row() as f32 * line_height - scroll_top,

        //                     line_height
        //                 });
        //             }
        //         }
        //     }

        //     scene.restore();

        //     // desenhar glifos
        //     scene.set_fill_style(FillStyle::Color(ColorU::black()));

        //     for (ix, line) in layout.line_layouts.iter().enumerate() {
        //         let row = start_row + ix as u32;

        //         let line_origin = vec2f(
        //             -scroll_left - descent,
        //             row as f32 * line_height - scroll_top
        //         );

        //         line.paint(
        //             line_origin,
        //             rect,
        //             &[(0..line.len, ColorU::black())],
        //             scene,
        //             font_cache
        //         );
        //     }

        //     for cursor in cursors {
        //         cursor.paint(scene);
        //     }

        //     scene.restore()
        // }
    }
}

impl Element for BufferElement {
    fn layout(
        &mut self,

        constraint: SizeConstraint,

        ctx: &mut LayoutContext,
        app: &AppContext
    ) -> Vector2F {
        let mut size = constraint.max;

        if size.y().is_infinite() {
            let view = self.view.as_ref(app);

            size.set_y((view.max_point(app).row() + 1) as f32 * view.line_height(ctx.font_cache));
        }

        if size.x().is_infinite() {
            unimplemented!("ainda não lidamos com uma restrição de largura infinita em elementos de buffer");
        }

        let view = self.view.as_ref(app);
        let font_cache = &ctx.font_cache;
        let layout_cache = &ctx.text_layout_cache;
        let line_height = view.line_height(font_cache);

        let gutter_padding;
        let gutter_width;

        if view.is_gutter_visible() {
            gutter_padding = view.em_width(ctx.font_cache);

            match view.max_line_number_width(ctx.font_cache, ctx.text_layout_cache, app) {
                Err(error) => {
                    log::error!("erro ao calcular a largura máxima do número de linha: {}", error);

                    return size;
                }

                Ok(width) => gutter_width = width + gutter_padding * 2.0,
            }
        } else {
            gutter_padding = 0.0;
            gutter_width = 0.0
        };

        let gutter_size = vec2f(gutter_width, size.y());
        let text_size = size - vec2f(gutter_width, 0.0);

        let autoscroll_horizontally = view.autoscroll_vertically(size.y(), line_height, app);

        let line_number_layouts = if view.is_gutter_visible() {
            match view.layout_line_numbers(size.y(), ctx.font_cache, ctx.text_layout_cache, app) {
                Err(error) => {
                    log::error!("erro ao definir números de linha: {}", error);
                    return size;
                }

                Ok(layouts) => layouts
            }
        } else {
            Vec::new()
        };

        let start_row = view.scroll_position().y() as u32;
        let scroll_top = view.scroll_position().y() * line_height;
        let end_row = ((scroll_top + size.y()) / line_height).ceil() as u32 + 1; // adicionar 1 para garantir que as seleções saiam da tela

        let mut max_visible_line_width = 0.0;

        let line_layouts = match view.layout_lines(start_row..end_row, font_cache, layout_cache, app) {
            Err(error) => {
                log::error!("erro ao traçar linhas: {}", error);

                return size;
            }

            Ok(layouts) => {
                for line in &layouts {
                    if line.width > max_visible_line_width {
                        max_visible_line_width = line.width;
                    }
                }

                layouts
            }
        };

        self.layout = Some(LayoutState {
            size,
            gutter_size,
            gutter_padding,
            text_size,
            line_layouts,
            line_number_layouts,
            max_visible_line_width,
            autoscroll_horizontally,
        });

        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &mut MutableAppContext) {
        let layout = self.layout.as_ref().unwrap();

        let view = self.view.as_ref(app);

        view.clamp_scroll_left(
            layout
                .scroll_max(view, ctx.font_cache, ctx.text_layout_cache, app.ctx())
                .x()
        );

        if layout.autoscroll_horizontally {
            view.autoscroll_horizontally(
                view.scroll_position().y() as u32,

                layout.text_size.x(),
                layout.scroll_width(view, ctx.font_cache, ctx.text_layout_cache, app.ctx()),

                view.em_width(ctx.font_cache),
                &layout.line_layouts,

                app.ctx()
            );
        }
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let rect;
        let gutter_rect;
        let text_rect;

        {
            let layout = self.layout.as_ref().unwrap();

            rect = RectF::new(origin, layout.size);
            gutter_rect = RectF::new(origin, layout.gutter_size);

            text_rect = RectF::new(
                origin + vec2f(layout.gutter_size.x(), 0.0),
                layout.text_size
            );
        }

        if self.view.as_ref(app).is_gutter_visible() {
            self.paint_gutter(gutter_rect, ctx, app);
        }

        self.paint_text(text_rect, ctx, app);

        self.paint = Some(PaintState { rect, text_rect });
    }

    fn dispatch_event(&self, event: &Event, ctx: &mut EventContext, app: &AppContext) -> bool {
        match event {
            Event::LeftMouseDown { position, cmd } => self.mouse_down(*position, *cmd, ctx, app),
            Event::LeftMouseUp { position } => self.mouse_up(*position, ctx, app),
            Event::LeftMouseDragged { position } => self.mouse_dragged(*position, ctx, app),

            Event::ScrollWheel {
                position,
                delta,
                precise
            } => self.scroll(*position, *delta, *precise, ctx, app),

            Event::KeyDown { chars, .. } => self.key_down(chars, ctx, app)
        }
    }

    fn size(&self) -> Option<Vector2F> {
        self.layout.as_ref().map(|layout| layout.size)
    }
}

struct LayoutState {
    size: Vector2F,

    gutter_size: Vector2F,
    gutter_padding: f32,

    text_size: Vector2F,

    line_layouts: Vec<Arc<text_layout::Line>>,
    line_number_layouts: Vec<Arc<text_layout::Line>>,

    max_visible_line_width: f32,
    autoscroll_horizontally: bool
}

impl LayoutState {
    fn scroll_width(
        &self,

        view: &BufferView,

        font_cache: &FontCache,
        layout_cache: &TextLayoutCache,

        app: &AppContext
    ) -> f32 {
        let row = view.rightmost_point(app).row();

        let longest_line_width = view
            .layout_line(row, font_cache, layout_cache, app)
            .unwrap()
            .width;

        longest_line_width.max(self.max_visible_line_width) + view.em_width(font_cache)
    }

    fn scroll_max(
        &self,
        view: &BufferView,

        font_cache: &FontCache,
        layout_cache: &TextLayoutCache,

        app: &AppContext
    ) -> Vector2F {
        vec2f(
            ((self.scroll_width(view, font_cache, layout_cache, app) - self.text_size.x()) / view.em_width(font_cache))
            .max(0.0),
            view.max_point(app).row().saturating_sub(1) as f32
        )
    }
}

struct PaintState {
    rect: RectF,
    text_rect: RectF
}

impl PaintState {
    fn point_for_position(
        &self,
        view: &BufferView,
        layout: &LayoutState,
        position: Vector2F,
        font_cache: &FontCache,
        app: &AppContext
    ) -> DisplayPoint {
        let scroll_position = view.scroll_position();
        let position = position - self.text_rect.origin();

        let y = position.y().max(0.0).min(layout.size.y());

        let row = ((y / view.line_height(font_cache)) + scroll_position.y()) as u32;
        let row = cmp::min(row, view.max_point(app).row());

        let line = &layout.line_layouts[(row - scroll_position.y() as u32) as usize];

        let x = position.x() + (scroll_position.x() * view.em_width(font_cache));

        let column = if x >= 0.0 {
            line.index_for_x(x)
                .map(|ix| ix as u32)
                .unwrap_or(view.line_len(row, app).unwrap())
        } else {
            0
        };

        DisplayPoint::new(row, column)
    }
}

struct Cursor {
    x: f32,
    y: f32,

    line_height: f32
}

impl Cursor {
    fn paint(&self, scene: &mut Scene) {
        // scene.set_fill_style(FillStyle::Color(ColorU::black()));
        //
        // scene.fill_rect(RectF::new(
        //     vec2f(self.x, self.y),
        //     vec2f(2.0, self.line_height)
        // ));
    }
}

#[derive(Debug)]
struct Selection {
    start_y: f32,

    line_height: f32,
    lines: Vec<SelectionLine>
}

#[derive(Debug)]
struct SelectionLine {
    start_x: f32,
    end_x: f32
}

impl Selection {
    fn paint(&self, scene: &mut Scene) {
        if self.lines.len() >= 2 && self.lines[0].start_x > self.lines[1].end_x {
            self.paint_lines(self.start_y, &self.lines[0..1], scene);
            self.paint_lines(self.start_y + self.line_height, &self.lines[1..], scene);
        } else {
            self.paint_lines(self.start_y, &self.lines, scene);
        }
    }

    fn paint_lines(&self, start_y: f32, lines: &[SelectionLine], scene: &mut Scene) {
        // use Direction::*;
        //
        // if lines.is_empty() {
        //     return;
        // }
        //
        // let mut path = Path2D::new();
        // let corner_radius = 0.08 * self.line_height;
        //
        // let first_line = lines.first().unwrap();
        // let last_line = lines.last().unwrap();
        //
        // let corner = vec2f(first_line.end_x, start_y);
        //
        // path.move_to(corner - vec2f(corner_radius, 0.0));
        // rounded_corner(&mut path, corner, corner_radius, Right, Down);
        //
        // let mut iter = lines.iter().enumerate().peekable();
        //
        // while let Some((ix, line)) = iter.next() {
        //     let corner = vec2f(line.end_x, start_y + (ix + 1) as f32 * self.line_height);
        //
        //     if let Some((_, next_line)) = iter.peek() {
        //         let next_corner = vec2f(next_line.end_x, corner.y());
        //
        //         match next_corner.x().partial_cmp(&corner.x()).unwrap() {
        //             Ordering::Equal => {
        //                 path.line_to(corner);
        //             }
        //
        //             Ordering::Less => {
        //                 path.line_to(corner - vec2f(0.0, corner_radius));
        //                 rounded_corner(&mut path, corner, corner_radius, Down, Left);
        //
        //                 path.line_to(next_corner + vec2f(corner_radius, 0.0));
        //                 rounded_corner(&mut path, next_corner, corner_radius, Left, Down);
        //             }
        //
        //             Ordering::Greater => {
        //                 path.line_to(corner - vec2f(0.0, corner_radius));
        //                 rounded_corner(&mut path, corner, corner_radius, Down, Right);
        //
        //                 path.line_to(next_corner - vec2f(corner_radius, 0.0));
        //                 rounded_corner(&mut path, next_corner, corner_radius, Right, Down);
        //             }
        //         }
        //     } else {
        //         path.line_to(corner - vec2f(0.0, corner_radius));
        //         rounded_corner(&mut path, corner, corner_radius, Down, Left);
        //
        //         let corner = vec2f(line.start_x, corner.y());
        //         path.line_to(corner + vec2f(corner_radius, 0.0));
        //
        //         rounded_corner(&mut path, corner, corner_radius, Left, Up);
        //     }
        // }
        //
        // if first_line.start_x > last_line.start_x {
        //     let corner = vec2f(last_line.start_x, start_y + self.line_height);
        //     path.line_to(corner + vec2f(0.0, corner_radius));
        //
        //     rounded_corner(&mut path, corner, corner_radius, Up, Right);
        //     let corner = vec2f(first_line.start_x, corner.y());
        //
        //     path.line_to(corner - vec2f(corner_radius, 0.0));
        //     rounded_corner(&mut path, corner, corner_radius, Right, Up);
        // }
        //
        // let corner = vec2f(first_line.start_x, start_y);
        // path.line_to(corner + vec2f(0.0, corner_radius));
        //
        // rounded_corner(&mut path, corner, corner_radius, Up, Right);
        // path.close_path();
        //
        // scene.set_fill_style(FillStyle::Color(
        //     ColorF::new(0.639, 0.839, 1.0, 1.0).to_u8()
        // ));
        //
        // scene.fill_path(path, FillRule::Winding);
    }
}

enum Direction {
    Up,
    Down,
    Left,
    Right
}

fn rounded_corner(
    path: &mut Path2D,
    corner: Vector2F,
    radius: f32,

    incoming: Direction,
    outgoing: Direction
) {
    use std::f32::consts::PI;
    use Direction::*;

    match (incoming, outgoing) {
        (Down, Right) => path.arc(
            corner + vec2f(radius, -radius),
            radius,

            1.0 * PI,
            0.5 * PI,

            ArcDirection::CCW
        ),

        (Down, Left) => path.arc(
            corner + vec2f(-radius, -radius),
            radius,

            0.0,
            0.5 * PI,

            ArcDirection::CW
        ),

        (Up, Right) => path.arc(
            corner + vec2f(radius, radius),
            radius,

            1.0 * PI,
            1.5 * PI,

            ArcDirection::CW
        ),

        (Up, Left) => path.arc(
            corner + vec2f(-radius, radius),
            radius,

            0.0,
            1.5 * PI,

            ArcDirection::CCW
        ),

        (Right, Up) => path.arc(
            corner + vec2f(-radius, -radius),
            radius,

            0.5 * PI,
            0.0,

            ArcDirection::CCW
        ),

        (Right, Down) => path.arc(
            corner + vec2f(-radius, radius),
            radius,

            1.5 * PI,
            2.0 * PI,

            ArcDirection::CW
        ),

        (Left, Up) => path.arc(
            corner + vec2f(radius, -radius),
            radius,

            0.5 * PI,
            PI,

            ArcDirection::CW
        ),

        (Left, Down) => path.arc(
            corner + vec2f(radius, radius),
            radius,

            1.5 * PI,
            PI,

            ArcDirection::CCW
        ),

        _ => panic!("direções de entrada e saída inválidas para uma esquina"),
    }
}

fn scale_vertical_mouse_autoscroll_delta(delta: f32) -> f32 {
    delta.powf(1.5) / 100.0
}

fn scale_horizontal_mouse_autoscroll_delta(delta: f32) -> f32 {
    delta.powf(1.2) / 300.0
}