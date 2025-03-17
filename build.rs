const COMMANDS: &[&str] = &["start_recording", "stop_recording"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
