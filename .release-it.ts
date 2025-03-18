import type { Config } from "release-it";

export default {
  git: {
    commitMessage: "tauri-plugin-mic-recorder-api v${version}",
    tagName: "v${version}",
  },
  npm: {
    publish: true,
  },
} satisfies Config;
