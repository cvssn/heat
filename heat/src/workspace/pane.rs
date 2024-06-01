use super::{ItemViewHandle, SplitDirection};
use crate::{settings::Settings, watch};
use gpui::{color::ColorU, elements::*, keymap::Binding, App, AppContext, Border, Entity, View, ViewContext};
use std::cmp;

pub fn init(app: &mut App) {
    app.add_action("pane:activate_item", |pane: &mut Pane, index: &usize, ctx| {
        pane.activate_item(*index, ctx);
    });

    app.add_action("pane:activate_prev_item", |pane: &mut Pane, _: &(), ctx| {
        pane.activate_prev_item(ctx);
    });

    app.add_action("pane:activate_next_item", |pane: &mut Pane, _: &(), ctx| {
        pane.activate_next_item(ctx);
    });

    app.add_action("pane:close_active_item", |pane: &mut Pane, _: &(), ctx| {
        pane.close_active_item(ctx);
    });

    app.add_action("pane:split_up", |pane: &mut Pane, _: &(), ctx| {
        pane.split(SplitDirection::Up, ctx);
    });

    app.add_action("pane:split_down", |pane: &mut Pane, _: &(), ctx| {
        pane.split(SplitDirection::Down, ctx);
    });

    app.add_action("pane:split_left", |pane: &mut Pane, _: &(), ctx| {
        pane.split(SplitDirection::Left, ctx);
    });

    app.add_action("pane:split_right", |pane: &mut Pane, _: &(), ctx| {
        pane.split(SplitDirection::Right, ctx);
    });

    app.add_bindings(vec![
        Binding::new("shift-cmd-{", "pane:activate_prev_item", Some("Pane")),
        Binding::new("shift-cmd-}", "pane:activate_next_item", Some("Pane")),
        
        Binding::new("cmd-w", "pane:close_active_item", Some("Pane")),
        Binding::new("cmd-k up", "pane:split_up", Some("Pane")),
        Binding::new("cmd-k down", "pane:split_down", Some("Pane")),
        Binding::new("cmd-k left", "pane:split_left", Some("Pane")),
        Binding::new("cmd-k right", "pane:split_right", Some("Pane"))
    ]);
}

pub enum Event {
    Activate,
    Remove,

    Split(SplitDirection)
}

#[derive(Debug, Eq, PartialEq)]
pub struct State {
    pub tabs: Vec<TabState>
}

#[derive(Debug, Eq, PartialEq)]
pub struct TabState {
    pub title: String,
    pub active: bool
}

pub struct Pane {
    items: Vec<Box<dyn ItemViewHandle>>,
    active_item: usize,
    settings: watch::Receiver<Settings>
}

impl Pane {
    pub fn new(settings: watch::Receiver<Settings>) -> Self {
        Self {
            items: Vec::new(),
            active_item: 0,
            settings
        }
    }

    pub fn activate(&self, ctx: &mut ViewContext<Self>) {
        ctx.emit(Event::Activate);
    }

    pub fn add_item(
        &mut self,

        item: Box<dyn ItemViewHandle>,
        ctx: &mut ViewContext<Self>
    ) -> usize {
        let item_idx = cmp::min(self.active_item + 1, self.items.len());
        
        self.items.insert(item_idx, item);
        ctx.notify();

        item_idx
    }

    #[cfg(test)]
    pub fn items(&self) -> &[Box<dyn ItemViewHandle>] {
        &self.items
    }

    pub fn active_item(&self) -> Option<Box<dyn ItemViewHandle>> {
        self.items.get(self.active_item).cloned()
    }

    pub fn activate_entry(
        &mut self,

        entry_id: (usize, usize),
        ctx: &mut ViewContext<Self>
    ) -> bool {
        if let Some(index) = self
            .items
            .iter()
            .position(|item| item.entry_id(ctx.app()).map_or(false, |id| id == entry_id))
        {
            self.activate_item(index, ctx);

            true
        } else {
            false
        }
    }

    pub fn item_index(&self, item: &dyn ItemViewHandle) -> Option<usize> {
        self.items.iter().position(|i| i.id() == item.id())
    }

    pub fn activate_item(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if index < self.items.len() {
            self.active_item = index;
            self.focus_active_item(ctx);

            ctx.notify();
        }
    }

    pub fn activate_prev_item(&mut self, ctx: &mut ViewContext<Self>) {
        if self.active_item > 0 {
            self.active_item -= 1;
        } else {
            self.active_item = self.items.len() - 1;
        }

        self.focus_active_item(ctx);

        ctx.notify();
    }

    pub fn activate_next_item(&mut self, ctx: &mut ViewContext<Self>) {
        if self.active_item + 1 < self.items.len() {
            self.active_item += 1;
        } else {
            self.active_item = 0;
        }

        self.focus_active_item(ctx);

        ctx.notify();
    }

    pub fn close_active_item(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.items.is_empty() {
            self.items.remove(self.active_item);

            if self.active_item >= self.items.len() {
                self.active_item = self.items.len().saturating_sub(1);
            }

            ctx.notify();
        }

        if self.items.is_empty() {
            ctx.emit(Event::Remove);
        }
    }

    fn focus_active_item(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(active_item) = self.active_item() {
            ctx.focus(active_item.to_any());
        }
    }

    pub fn split(&mut self, direction: SplitDirection, ctx: &mut ViewContext<Self>) {
        ctx.emit(Event::Split(direction));
    }

    fn render_tabs<'a>(&self, app: &AppContext) -> Box<dyn Element> {
        let settings = smol::block_on(self.settings.read());
        let border_color = ColorU::new(0xdb, 0xdb, 0xdc, 0xff);

        let mut row = Flex::row();

        let last_item_ix = self.items.len() - 1;

        for (ix, item) in self.items.iter().enumerate() {
            let title = item.title(app);

            let mut border = Border::new(1.0, border_color);

            border.left = ix > 0;
            border.right = ix == last_item_ix;
            border.bottom = ix != self.active_item;

            let mut container = Container::new(
                Align::new(
                    Label::new(title, settings.ui_font_family, settings.ui_font_size).boxed()
                ).boxed()
            ).with_uniform_padding(6.0).with_border(border);

            if ix == self.active_item {
                container = container
                    .with_background_color(ColorU::white())
                    .with_overdraw_bottom(1.5);
            } else {
                container = container.with_background_color(ColorU::new(0xea, 0xea, 0xeb, 0xff));
            }

            row.add_child(
                Expanded::new(
                    1.0,

                    ConstrainedBox::new(
                        EventHandler::new(container.boxed())
                            .on_mouse_down(move |ctx, _| {
                                ctx.dispatch_action("pane:activate_item", ix);

                                true
                            }).boxed()
                    ).with_max_width(264.0).boxed()
                ).boxed()
            );
        }

        row.add_child(
            Expanded::new(
                1.0,

                Container::new(
                    LineBox::new(
                        settings.ui_font_family,
                        settings.ui_font_size,

                        Empty::new().boxed()
                    ).boxed()
                )
                .with_uniform_padding(6.0)
                .with_border(Border::bottom(1.0, border_color))
                .boxed()
            ).boxed()
        );

        row.boxed()
    }
}

impl Entity for Pane {
    type Event = Event;
}

impl View for Pane {
    fn ui_name() -> &'static str {
        "pane"
    }

    fn render<'a>(&self, app: &AppContext) -> Box<dyn Element> {
        if let Some(active_item) = self.active_item() {
            Flex::column()
                .with_child(self.render_tabs(app))
                .with_child(Expanded::new(1.0, ChildView::new(active_item.id()).boxed()).boxed())
                .boxed()
        } else {
            Empty::new().boxed()
        }
    }

    fn on_focus(&mut self, ctx: &mut ViewContext<Self>) {
        self.focus_active_item(ctx);
    }

    // fn state(&self, app: &AppContext) -> Self::State {
    //     State {
    //         tabs: self
    //             .items
    //             .iter()
    //             .enumerate()
    //             .map(|(idx, item)| TabState {
    //                 title: item.title(app),
    //                 active: idx == self.active_item,
    //             })
    //             .collect()
    //     }
    // }
}