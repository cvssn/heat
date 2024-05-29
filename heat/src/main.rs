use std::fs;

use fs::OpenOptions;
use gpui::platform::{current as platform, App as _};
use log::LevelFilter;
use simplelog::SimpleLogger;

fn main() {
    init_logger();

    platform::app()
        .on_finish_launching(|| log::info!("finalizando lançamento"))
        .run();
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