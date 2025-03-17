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
              setIsRecording(true);

              await startRecording();
            } catch (error) {
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
              setIsRecording(false);

              setTimeout(() => {
                setSavePath(path);
              }, 200);
            } catch (error) {
              message.error(String(error));
            }
          }}
        >
          Stop Recording
        </Button>
      </Space>

      {!isRecording && savePath && (
        <audio
          controls
          src={convertFileSrc(savePath)}
          onError={(error) => {
            console.log("error", error);
          }}
        >
          {savePath}
        </audio>
      )}
    </Space>
  );
};

export default App;
