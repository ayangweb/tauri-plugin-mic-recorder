# tauri-plugin-mic-recorder

> This plugin only works on tauri v2, if you need the v1 plugin, feel free to submit a PR!

Supports recording audio using a microphone and saving the recorded data as a file.

https://github.com/user-attachments/assets/7c6f1df4-96e6-4cac-806b-098e8bccc1f7

## Platform Support

| Platform | Supported |
| -------- | --------- |
| Windows  | ✅        |
| macOS    | ✅        |
| Linux    | ✅        |
| Android  | ✅        |
| iOS      | ✅        |

## Install

```shell
cargo add tauri-plugin-mic-recorder
```

You can install the JavaScript Guest bindings using your preferred JavaScript package manager:

```shell
pnpm add tauri-plugin-mic-recorder-api
```

## Usage

`src-tauri/src/lib.rs`

```diff
pub fn run() {
    tauri::Builder::default()
+       .plugin(tauri_plugin_mic_recorder::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`src-tauri/capabilities/default.json`

```diff
{
    ...
    "permissions": [
        ...
+       "mic-recorder:default"
    ]
}
```

Afterwards all the plugin's APIs are available through the JavaScript guest bindings:

```ts
import { startRecording } from "tauri-plugin-mic-recorder-api";

startRecording();
```

## Methods

| Method           | Description             |
| ---------------- | ----------------------- |
| `startRecording` | Starts recording audio. |
| `stopRecording`  | Stops recording audio.  |

## Example

```shell
git clone https://github.com/ayangweb/tauri-plugin-mic-recorder.git
```

```shell
pnpm install

pnpm build

cd examples/tauri-app

pnpm install

pnpm dev
```

## Thanks

- Use [cpal](https://github.com/RustAudio/cpal) and [hound](https://github.com/ruuda/hound) record and generate wav files.

## Who's Use It

- [Coco AI](https://github.com/infinilabs/coco-app) - Search, Connect, Collaborate, Your Personal AI Search and Assistant, all in one space.
