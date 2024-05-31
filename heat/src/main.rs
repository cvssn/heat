use fs::OpenOptions;

use gpui::{
    executor,
    geometry::{rect::RectF, vector::vec2f},
    platform::{current as platform, App as _, Runner as _, WindowOptions},
    FontCache
};

use log::LevelFilter;
use simplelog::SimpleLogger;
use std::{fs, mem, rc::Rc, sync::Arc};

use zed::{
    editor, settings,

    workspace::{self, OpenParams}
};

fn main() {
    init_logger();

    let platform = Arc::new(platform::app());

    let foreground = Rc::new(
        executor::Foreground::platform(platform.dispatcher())
            .expect("o foreground não conseguiu criar o executor")
    );

    let font_cache = FontCache::new();

    let (settings_tx, settings_rx) = settings::channel(&font_cache).unwrap();

    let mut app = gpui::App::new().unwrap();

    platform::runner()
        .on_finish_launching(move || {
            log::info!("lançamento finalizado");

            workspace::init(&mut app);
            editor::init(&mut app);
            
            if stdout_is_a_pty() {
                platform.activate(true);
            }

            let paths = std::env::args()
                .skip(1)
                .filter_map(|arg| match fs::canonicalize(arg) {
                    Ok(path) => Some(path),

                    Err(error) => {
                        log::error!("erro ao analisar o argumento do caminho: {}", error);

                        None
                    }
                }).collect::<Vec<_>>();

            if !paths.is_empty() {
                app.dispatch_global_action(
                    "workspace:open_paths",

                    OpenParams {
                        paths,
                        settings: settings_rx,
                    }
                );

                mem::forget(app);
            }

            // let window = platform
            //     .open_window(
            //         WindowOptions {
            //             bounds: RectF::new(vec2f(0., 0.), vec2f(1024., 768.)),
            //             title: Some("Heat")
            //         },

            //         foreground
            //     )
                
            //     .expect("erro ao abrir a janela");

            // mem::forget(window); // vazar janela por agora para não fechar
        }).run();
}

fn init_logger() {
    let level = LevelFilter::Info;

    if stdout_is_a_pty() {
        SimpleLogger::init(level, Default::default()).expect("não foi possível inicializar o logger");
    } else {
        let log_dir_path = dirs::home_dir()
            .expect("não foi possível localizar o diretório base para logging")
            .join("Library/Logs/");

        let log_file_path = log_dir_path.join("Head.log");

        fs::create_dir_all(&log_dir_path).expect("não foi possível criar um diretório log");

        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path)
            .expect("não foi possível abrir o logfile");

        simplelog::WriteLogger::init(level, simplelog::Config::default(), log_file)
            .expect("não foi possível inicializar o logger");
    }
}

fn stdout_is_a_pty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO as i32) != 0 }
}