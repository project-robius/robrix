// this stub is necessary because some platforms require building
// as dll (mobile / wasm) and some require to be built as executable
// unfortunately cargo doesn't facilitate this without a main.rs stub
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
fn main() {
    robrix::app::app_main()
}
