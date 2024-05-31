use super::{pane, Pane, PaneGroup, SplitDirection, Workspace};
use crate::{settings::Settings, watch};
use gpui::{color::rgbu, ChildView};

use gpui::{
    elements::*, AnyViewHandle, AppContext, Entity, ModelHandle, MutableAppContext, View,
    ViewContext, ViewHandle
};

use log::{error, info};
use std::{collections::HashSet, path::PathBuf};

pub trait ItemView: View {
    fn is_activate_event(event: &Self::Event) -> bool;
    fn title(&self, app: &AppContext) -> String;
    fn entry_id(&self, app: &AppContext) -> Option<(usize, usize)>;

    fn clone_on_split(&self, _: &mut ViewContext<Self>) -> Option<Self>
    where
        Self: Sized
    {
        None
    }
}

pub trait ItemViewHandle: Send + Sync {
    fn title(&self, app: &AppContext) -> String;
    fn entry_id(&self, app: &AppContext) -> Option<(usize, usize)>;
    fn boxed_clone(&self) -> Box<dyn ItemViewHandle>;
    fn clone_on_split(&self, app: &mut MutableAppContext) -> Option<Box<dyn ItemViewHandle>>;
    fn set_parent_pane(&self, pane: &ViewHandle<Pane>, app: &mut MutableAppContext);
    
    fn id(&self) -> usize;
    fn to_any(&self) -> AnyViewHandle;
}

impl<T: ItemView> ItemViewHandle for ViewHandle<T> {
    fn title(&self, app: &AppContext) -> String {
        self.as_ref(app).title(app)
    }

    fn entry_id(&self, app: &AppContext) -> Option<(usize, usize)> {
        self.as_ref(app).entry_id(app)
    }

    fn boxed_clone(&self) -> Box<dyn ItemViewHandle> {
        Box::new(self.clone())
    }

    fn clone_on_split(&self, app: &mut MutableAppContext) -> Option<Box<dyn ItemViewHandle>> {
        self.update(app, |item, ctx| {
            ctx.add_option_view(|ctx| item.clone_on_split(ctx))
        }).map(|handle| Box::new(handle) as Box<dyn ItemViewHandle>)
    }

    fn set_parent_pane(&self, pane: &ViewHandle<Pane>, app: &mut MutableAppContext) {
        pane.update(app, |_, ctx| {
            ctx.subscribe_to_view(self, |pane, item, event, ctx| {
                if T::is_activate_event(event) {
                    if let Some(ix) = pane.item_index(&item) {
                        pane.activate_item(ix, ctx);

                        pane.activate(ctx);
                    }
                }
            })
        })
    }

    fn id(&self) -> usize {
        self.id()
    }

    fn to_any(&self) -> AnyViewHandle {
        self.into()
    }
}

impl Clone for Box<dyn ItemViewHandle> {
    fn clone(&self) -> Box<dyn ItemViewHandle> {
        self.boxed_clone()
    }
}

#[derive(Debug)]
pub struct State {
    pub modal: Option<usize>,
    pub center: PaneGroup
}

pub struct WorkspaceView {
    pub workspace: ModelHandle<Workspace>,
    pub settings: watch::Receiver<Settings>,

    modal: Option<AnyViewHandle>,
    center: PaneGroup,
    panes: Vec<ViewHandle<Pane>>,
    active_pane: ViewHandle<Pane>,
    loading_entries: HashSet<(usize, usize)>
}

impl WorkspaceView {
    pub fn new(
        workspace: ModelHandle<Workspace>,
        settings: watch::Receiver<Settings>,
        ctx: &mut ViewContext<Self>
    ) -> Self {
        ctx.observe(&workspace, Self::workspace_updated);

        let pane = ctx.add_view(|_| Pane::new(settings.clone()));
        let pane_id = pane.id();

        ctx.subscribe_to_view(&pane, move |me, _, event, ctx| {
            me.handle_pane_event(pane_id, event, ctx)
        });

        ctx.focus(&pane);

        WorkspaceView {
            workspace,
            modal: None,
            center: PaneGroup::new(pane.id()),
            panes: vec![pane.clone()],
            active_pane: pane.clone(),
            loading_entries: HashSet::new(),
            settings
        }
    }

    pub fn contains_paths(&self, paths: &[PathBuf], app: &AppContext) -> bool {
        self.workspace.as_ref(app).contains_paths(paths, app)
    }

    pub fn open_paths(&self, paths: &[PathBuf], app: &mut MutableAppContext) {
        self.workspace.update(app, |workspace, ctx| workspace.open_paths(paths, ctx));
    }

    pub fn toggle_modal<V, F>(&mut self, ctx: &mut ViewContext<Self>, add_view: F)
    where
        V: 'static + View,

        F: FnOnce(&mut ViewContext<Self>, &mut Self) -> ViewHandle<V>
    {
        if self.modal.as_ref().map_or(false, |modal| modal.is::<V>()) {
            self.modal.take();

            ctx.focus_self();
        } else {
            let modal = add_view(ctx, self);

            ctx.focus(&modal);

            self.modal = Some(modal.into());
        }

        ctx.notify();
    }

    pub fn modal(&self) -> Option<&AnyViewHandle> {
        self.modal.as_ref()
    }

    pub fn dismiss_modal(&mut self, ctx: &mut ViewContext<Self>) {
        if self.modal.take().is_some() {
            ctx.focus(&self.active_pane);

            ctx.notify();
        }
    }

    pub fn open_entry(&mut self, entry: (usize, usize), ctx: &mut ViewContext<Self>) {
        if self.loading_entries.contains(&entry) {
            return;
        }

        if self
            .active_pane()
            .update(ctx, |pane, ctx| pane.activate_entry(entry, ctx))
        {
            return;
        }

        self.loading_entries.insert(entry);

        match self
            .workspace
            .update(ctx, |workspace, ctx| workspace.open_entry(entry, ctx))
        {
            Err(error) => error!("{}", error),

            Ok(item) => {
                let settings = self.settings.clone();

                let _ = ctx.spawn(item, move |me, item, ctx| {
                    me.loading_entries.remove(&entry);

                    match item {
                        Ok(item) => {
                            let item_view = item.add_view(ctx.window_id(), settings, ctx.app_mut());
                            
                            me.add_item(item_view, ctx);
                        }

                        Err(error) => {
                            error!("{}", error);
                        }
                    }
                });
            }
        }
    }

    pub fn open_example_entry(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(tree) = self.workspace.as_ref(ctx).worktrees().iter().next() {
            if let Some(file) = tree.as_ref(ctx).files().next() {
                info!("open_entry ({}, {})", tree.id(), file.entry_id);

                self.open_entry((tree.id(), file.entry_id), ctx);
            } else {
                error!("nenhum arquivo de exemplo encontrado para worktree {}", tree.id());
            }
        } else {
            error!("nenhuma árvore de trabalho encontrada ao abrir a entrada de exemplo");
        }
    }

    fn workspace_updated(&mut self, _: ModelHandle<Workspace>, ctx: &mut ViewContext<Self>) {
        ctx.notify();
    }

    fn add_pane(&mut self, ctx: &mut ViewContext<Self>) -> ViewHandle<Pane> {
        let pane = ctx.add_view(|_| Pane::new(self.settings.clone()));
        let pane_id = pane.id();

        ctx.subscribe_to_view(&pane, move |me, _, event, ctx| {
            me.handle_pane_event(pane_id, event, ctx)
        });

        self.panes.push(pane.clone());
        self.activate_pane(pane.clone(), ctx);

        pane
    }

    fn activate_pane(&mut self, pane: ViewHandle<Pane>, ctx: &mut ViewContext<Self>) {
        self.active_pane = pane;

        ctx.focus(&self.active_pane);
        ctx.notify();
    }

    fn handle_pane_event(
        &mut self,

        pane_id: usize,
        event: &pane::Event,
        ctx: &mut ViewContext<Self>
    ) {
        if let Some(pane) = self.pane(pane_id) {
            match event {
                pane::Event::Split(direction) => {
                    self.split_pane(pane, *direction, ctx);
                }

                pane::Event::Remove => {
                    self.remove_pane(pane, ctx);
                }

                pane::Event::Activate => {
                    self.activate_pane(pane, ctx);
                }
            }
        } else {
            error!("pane {} não encontrada", pane_id);
        }
    }

    fn split_pane(
        &mut self,

        pane: ViewHandle<Pane>,
        direction: SplitDirection,
        ctx: &mut ViewContext<Self>
    ) -> ViewHandle<Pane> {
        let new_pane = self.add_pane(ctx);

        self.activate_pane(new_pane.clone(), ctx);

        if let Some(item) = pane.as_ref(ctx).active_item() {
            if let Some(clone) = item.clone_on_split(ctx.app_mut()) {
                self.add_item(clone, ctx);
            }
        }

        self.center.split(pane.id(), new_pane.id(), direction).unwrap();

        ctx.notify();

        new_pane
    }

    fn remove_pane(&mut self, pane: ViewHandle<Pane>, ctx: &mut ViewContext<Self>) {
        if self.center.remove(pane.id()).unwrap() {
            self.panes.retain(|p| p != &pane);

            self.activate_pane(self.panes.last().unwrap().clone(), ctx);
        }
    }

    fn pane(&self, pane_id: usize) -> Option<ViewHandle<Pane>> {
        self.panes.iter().find(|pane| pane.id() == pane_id).cloned()
    }

    pub fn active_pane(&self) -> &ViewHandle<Pane> {
        &self.active_pane
    }

    fn add_item(&self, item: Box<dyn ItemViewHandle>, ctx: &mut ViewContext<Self>) {
        let active_pane = self.active_pane();

        item.set_parent_pane(&active_pane, ctx.app_mut());

        active_pane.update(ctx, |pane, ctx| {
            let item_idx = pane.add_item(item, ctx);

            pane.activate_item(item_idx, ctx);
        });
    }
}

impl Entity for WorkspaceView {
    type Event = ();
}

impl View for WorkspaceView {
    fn ui_name() -> &'static str {
        "workspace"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        Container::new(
            // self.center.render(bump)

            Stack::new()
                .with_child(self.center.render())
                .with_children(self.modal.as_ref().map(|m| ChildView::new(m.id()).boxed()))
                .boxed(),
        ).with_background_color(rgbu(0xea, 0xea, 0xeb)).boxed()
    }

    fn on_focus(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.active_pane);
    }
}

#[cfg(test)]
mod tests {
    use super::{pane, Workspace, WorkspaceView};
    use crate::{settings, test::temp_tree, workspace::WorkspaceHandle as _};
    use anyhow::Result;
    use gpui::{App, FontCache};
    use serde_json::json;

    #[test]
    fn test_open_entry() -> Result<()> {
        App::test(|mut app| async move {
            let dir = temp_tree(json!({
                "a": {
                    "aa": "conteúdos aa",
                    "ab": "conteúdos ab",
                    "ac": "conteúdos ab"
                }
            }));

            let settings = settings::channel(&FontCache::new()).unwrap().1;
            let workspace = app.add_model(|ctx| Workspace::new(vec![dir.path().into()], ctx));

            app.finish_pending_tasks().await; // abre e popula a árvore de trabalho
            let entries = workspace.file_entries(&app);

            let (_, workspace_view) = app.add_window(|ctx| WorkspaceView::new(workspace.clone(), settings, ctx));

            // abre a primeira entrada
            workspace_view.update(&mut app, |w, ctx| w.open_entry(entries[0], ctx));
            app.finish_pending_tasks().await;

            workspace_view.read(&app, |w, app| {
                assert_eq!(w.active_pane().as_ref(app).items().len(), 1);
            });

            // abre a segunda entrada
            workspace_view.update(&mut app, |w, ctx| w.open_entry(entries[1], ctx));
            app.finish_pending_tasks().await;

            workspace_view.read(&app, |w, app| {
                let active_pane = w.active_pane().as_ref(app);

                assert_eq!(active_pane.items().len(), 2);

                assert_eq!(
                    active_pane.active_item().unwrap().entry_id(app),

                    Some(entries[1])
                );
            });

            // abre a primeira entrada novamente
            workspace_view.update(&mut app, |w, ctx| w.open_entry(entries[0], ctx));

            app.finish_pending_tasks().await;

            workspace_view.read(&app, |w, app| {
                let active_pane = w.active_pane().as_ref(app);

                assert_eq!(active_pane.items().len(), 2);

                assert_eq!(
                    active_pane.active_item().unwrap().entry_id(app),
                    Some(entries[0])
                );
            });

            // abre a terceira entrada duas vezes simultaneamente
            workspace_view.update(&mut app, |w, ctx| {
                w.open_entry(entries[2], ctx);
                w.open_entry(entries[2], ctx);
            });

            app.finish_pending_tasks().await;

            workspace_view.read(&app, |w, app| {
                assert_eq!(w.active_pane().as_ref(app).items().len(), 3);
            });

            Ok(())
        })
    }

    #[test]
    fn test_pane_actions() -> Result<()> {
        App::test(|mut app| async move {
            pane::init(&mut app);

            let dir = temp_tree(json!({
                "a": {
                    "aa": "conteúdos aa",
                    "ab": "conteúdos ab",
                    "ac": "conteúdos ab"
                }
            }));

            let settings = settings::channel(&FontCache::new()).unwrap().1;
            let workspace = app.add_model(|ctx| Workspace::new(vec![dir.path().into()], ctx));

            app.finish_pending_tasks().await; // abre e popula a árvore de trabalho
            let entries = workspace.file_entries(&app);

            let (window_id, workspace_view) =
                app.add_window(|ctx| WorkspaceView::new(workspace.clone(), settings, ctx));

            workspace_view.update(&mut app, |w, ctx| w.open_entry(entries[0], ctx));
            app.finish_pending_tasks().await;

            let pane_1 = workspace_view.read(&app, |w, _| w.active_pane().clone());

            app.dispatch_action(window_id, vec![pane_1.id()], "pane:split_right", ());
            let pane_2 = workspace_view.read(&app, |w, _| w.active_pane().clone());
            assert_ne!(pane_1, pane_2);

            pane_2.read(&app, |p, app| {
                assert_eq!(p.active_item().unwrap().entry_id(app), Some(entries[0]));
            });

            app.dispatch_action(window_id, vec![pane_2.id()], "pane:close_active_item", ());

            workspace_view.read(&app, |w, _| {
                assert_eq!(w.panes.len(), 1);
                assert_eq!(w.active_pane(), &pane_1)
            });

            Ok(())
        })
    }
}