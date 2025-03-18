import { Button, message, Space } from "antd";
import { useState } from "react";
import { startRecording, stopRecording } from "tauri-plugin-mic-recorder-api";
import { convertFileSrc } from "@tauri-apps/api/core";

const App = () => {
  const [isRecording, setIsRecording] = useState<boolean>(false);
  const [savePath, setSavePath] = useState<string>("");

  return (
    <Space direction="vertical">
      <Space>
        <Button
          disabled={isRecording}
          onClick={async () => {
            try {
              await startRecording();
              setIsRecording(true);
            } catch (error) {
              setIsRecording(false);
              message.error(String(error));
            }
          }}
        >
          Start Recording
        </Button>
        <Button
          disabled={!isRecording}
          onClick={async () => {
            try {
              const path = await stopRecording();
              setSavePath(path);
            } catch (error) {
              message.error(String(error));
            } finally {
              setIsRecording(false);
            }
          }}
        >
          Stop Recording
        </Button>
      </Space>

      {!isRecording && savePath && (
        <audio controls src={convertFileSrc(savePath)}>
          <track kind="captions" />
        </audio>
      )}
    </Space>
  );
};

export default App;
