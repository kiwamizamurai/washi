#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = washi_core::cli::run(std::env::args().skip(1).collect()) {
        std::process::exit(code);
    }
    washi_lib::run()
}
