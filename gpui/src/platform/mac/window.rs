use crate::{
    executor,

    geometry::vector::Vector2F,
    platform::{self, Event}
};

use anyhow::{anyhow, Result};

use cocoa::{
    appkit::{
        NSBackingStoreBuffered, NSScreen, NSView, NSViewHeightSizable, NSViewWidthSizable,
        NSWindow, NSWindowStyleMask,
    },

    base::{id, nil},

    foundation::{NSAutoreleasePool, NSSize, NSString}
};

use ctor::ctor;

use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel, BOOL, NO, YES},
    sel, sel_impl
};

use smol::Timer;

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    mem, ptr,
    rc::Rc,
    time::{Duration, Instant}
};

use super::geometry::RectFExt;

const WINDOW_STATE_IVAR: &'static str = "windowState";

static mut WINDOW_CLASS: *const Class = ptr::null();
static mut VIEW_CLASS: *const Class = ptr::null();
static mut DELEGATE_CLASS: *const Class = ptr::null();

#[ctor]
unsafe fn build_classes() {
    WINDOW_CLASS = {
        let mut decl = ClassDecl::new("GPUIWindow", class!(NSWindow)).unwrap();

        decl.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR);
        decl.add_method(sel!(dealloc), dealloc_window as extern "C" fn(&Object, Sel));

        decl.add_method(
            sel!(canBecomeMainWindow),

            yes as extern "C" fn(&Object, Sel) -> BOOL
        );

        decl.add_method(
            sel!(canBecomeKeyWindow),

            yes as extern "C" fn(&Object, Sel) -> BOOL
        );

        decl.add_method(
            sel!(sendEvent:),

            send_event as extern "C" fn(&Object, Sel, id)
        );

        decl.register()
    };

    VIEW_CLASS = {
        let mut decl = ClassDecl::new("GPUIView", class!(NSView)).unwrap();

        decl.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR);
        decl.add_method(sel!(dealloc), dealloc_view as extern "C" fn(&Object, Sel));

        decl.add_method(
            sel!(keyDown:),

            handle_view_event as extern "C" fn(&Object, Sel, id)
        );

        decl.add_method(
            sel!(mouseDown:),

            handle_view_event as extern "C" fn(&Object, Sel, id)
        );

        decl.add_method(
            sel!(mouseUp:),

            handle_view_event as extern "C" fn(&Object, Sel, id)
        );

        decl.add_method(
            sel!(mouseDragged:),

            handle_view_event as extern "C" fn(&Object, Sel, id)
        );

        decl.add_method(
            sel!(scrollWheel:),

            handle_view_event as extern "C" fn(&Object, Sel, id)
        );

        decl.register()
    };

    DELEGATE_CLASS = {
        let mut decl = ClassDecl::new("GPUIWindowDelegate", class!(NSObject)).unwrap();

        decl.add_method(
            sel!(dealloc),

            dealloc_delegate as extern "C" fn(&Object, Sel)
        );

        decl.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR);

        decl.add_method(
            sel!(windowDidResize:),

            window_did_resize as extern "C" fn(&Object, Sel, id)
        );

        decl.register()
    };
}

pub struct Window(Rc<WindowState>);

struct WindowState {
    native_window: id,

    event_callback: RefCell<Option<Box<dyn FnMut(Event) -> bool>>>,
    resize_callback: RefCell<Option<Box<dyn FnMut(NSSize, f64)>>>,

    synthetic_drag_counter: Cell<usize>,

    executor: Rc<executor::Foreground>
}

impl Window {
    pub fn open(
        options: platform::WindowOptions,
        executor: Rc<executor::Foreground>
    ) -> Result<Self> {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);

            let frame = options.bounds.to_ns_rect();

            let style_mask = NSWindowStyleMask::NSClosableWindowMask
                | NSWindowStyleMask::NSMiniaturizableWindowMask
                | NSWindowStyleMask::NSResizableWindowMask
                | NSWindowStyleMask::NSTitledWindowMask;

            let native_window: id = msg_send![WINDOW_CLASS, alloc];

            let native_window = native_window.initWithContentRect_styleMask_backing_defer_(
                frame,
                style_mask,

                NSBackingStoreBuffered,

                NO
            );

            if native_window == nil {
                return Err(anyhow!("a janela retornou nulo (nil) do inicializador"));
            }

            let delegate: id = msg_send![DELEGATE_CLASS, alloc];
            let delegate = delegate.init();

            if native_window == nil {
                return Err(anyhow!("delegado retornou nulo (nil) do inicializador"));
            }

            native_window.setDelegate_(delegate);

            let native_view: id = msg_send![VIEW_CLASS, alloc];
            let native_view = NSView::init(native_view);

            if native_view == nil {
                return Err(anyhow!("view retorno nulo (nil) do inicializador"));
            }

            let window = Self(Rc::new(WindowState {
                native_window,

                event_callback: RefCell::new(None),
                resize_callback: RefCell::new(None),

                synthetic_drag_counter: Cell::new(0),

                executor
            }));

            (*native_window).set_ivar(
                WINDOW_STATE_IVAR,
                Rc::into_raw(window.0.clone()) as *const c_void
            );

            (*native_view).set_ivar(
                WINDOW_STATE_IVAR,
                Rc::into_raw(window.0.clone()) as *const c_void
            );

            (*delegate).set_ivar(
                WINDOW_STATE_IVAR,
                Rc::into_raw(window.0.clone()) as *const c_void
            );

            if let Some(title) = options.title.as_ref() {
                native_window.setTitle_(NSString::alloc(nil).init_str(title));
            }

            native_window.setAcceptsMouseMovedEvents_(YES);

            native_view.setAutoresizingMask_(NSViewWidthSizable | NSViewHeightSizable);
            native_view.setWantsBestResolutionOpenGLSurface_(YES);

            // da crate do winit: no mojave, as visualizações tornam-se automaticamente apoiadas
            // em camadas logo após serem adicionadas em um native_window. alterar o suporte de
            // camada de uma visualização quebra a associação entre a visualização e seu contexto
            // opengl associado
            //
            // para trabalhar com isso,
            // ao se fazer explicitamente o backup da camada de visualização antecipadamente, para
            // que o appkit não faça isso sozinho e quebre a associação com seu contexto
            native_view.setWantsLayer(YES);

            native_view.layer().setBackgroundColor_(
                msg_send![class!(NSColor), colorWithRed:1.0 green:0.0 blue:0.0 alpha:1.0]
            );

            native_window.setContentView_(native_view.autorelease());
            native_window.makeFirstResponder_(native_view);

            native_window.center();
            native_window.makeKeyAndOrderFront_(nil);

            pool.drain();

            Ok(window)
        }
    }

    pub fn zoom(&self) {
        unsafe {
            self.0.native_window.performZoom_(nil);
        }
    }

    pub fn size(&self) -> NSSize {
        self.0.size()
    }

    pub fn backing_scale_factor(&self) -> f64 {
        self.0.backing_scale_factor()
    }

    pub fn on_event<F: 'static + FnMut(Event) -> bool>(&mut self, callback: F) {
        *self.0.event_callback.borrow_mut() = Some(Box::new(callback));
    }

    pub fn on_resize<F: 'static + FnMut(NSSize, f64)>(&mut self, callback: F) {
        *self.0.resize_callback.borrow_mut() = Some(Box::new(callback));
    }
}