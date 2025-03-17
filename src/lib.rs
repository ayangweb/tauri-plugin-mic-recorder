use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

mod commands;

pub use commands::*;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("mic-recorder")
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording
        ])
        .build()
}
