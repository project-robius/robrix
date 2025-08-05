// this stub is necessary because some platforms require building
// as dll (mobile / wasm) and some require to be built as executable
// unfortunately cargo doesn't facilitate this without a main.rs stub

// This cfg option hides the command prompt console window on Windows.
// TODO: move this into Makepad itself as an addition to the `MAKEPAD` env var.
#![cfg_attr(all(feature = "hide_windows_console", target_os = "windows"), windows_subsystem = "windows")]

fn main() {
    robrix::app::app_main()
}
