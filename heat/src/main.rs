use fs::OpenOptions;
use gpui::platform::{current as platform, Runner as _};
use log::LevelFilter;
use simplelog::SimpleLogger;
use std::{fs, path::PathBuf};

use zed::{
    assets, editor, settings,

    workspace::{self, OpenParams}
};

fn main() {
    init_logger();

    let app = gpui::App::new(assets::Assets).unwrap();
    let (_, settings_rx) = settings::channel(&app.fonts()).unwrap();

    {
        let mut app = app.clone();

        platform::runner()
            .on_finish_launching(move || {
                workspace::init(&mut app);
                editor::init(&mut app);

                if stdout_is_a_pty() {
                    app.platform().activate(true);
                }

                let paths = collect_path_args();

                if !paths.is_empty() {
                    app.dispatch_global_action(
                        "workspace:open_paths",

                        OpenParams {
                            paths,

                            settings: settings_rx
                        }
                    );
                }
            }).run();
    }
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

fn collect_path_args() -> Vec<PathBuf> {
    std::env::args()
        .skip(1)
        .filter_map(|arg| match fs::canonicalize(arg) {
            Ok(path) => Some(path),

            Err(error) => {
                log::error!("erro ao analisar o argumento do caminho: {}", error);

                None
            }
        }).collect::<Vec<_>>()
}