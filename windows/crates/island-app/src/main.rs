#![windows_subsystem = "windows"]

fn main() {
    if let Err(error) = island_windows::run() {
        island_windows::show_error(&error.to_string());
    }
}
