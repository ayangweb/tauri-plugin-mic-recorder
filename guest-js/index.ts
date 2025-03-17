import { invoke } from "@tauri-apps/api/core";

export const COMMAND = {
  START_RECORDING: "plugin:mic-recorder|start_recording",
  STOP_RECORDING: "plugin:mic-recorder|stop_recording",
};

/**
 * Starts recording audio.
 *
 * @example
 * ```
 * import { startRecording } from 'tauri-plugin-mic-recorder-api';
 *
 * startRecording().then(() => {
 *   console.log("Recording started");
 * });
 * ```
 */
export const startRecording = () => {
  return invoke(COMMAND.START_RECORDING);
};

/**
 * Stops recording audio.
 *
 * @returns Returns the path where the recording file is stored.
 *
 * @example
 * ```
 * import { stopRecording } from 'tauri-plugin-mic-recorder-api';
 *
 * const savePath = await stopRecording();
 * console.log("Recording saved at:", savePath);
 * ```
 */
export const stopRecording = () => {
  return invoke<string>(COMMAND.STOP_RECORDING);
};
