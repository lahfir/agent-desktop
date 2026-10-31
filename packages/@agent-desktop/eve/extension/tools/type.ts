import { z } from "zod";

import { desktopTool, flag, ref, timeoutMs } from "../lib/tool";

export default desktopTool({
  description: "Insert text into a text field or editor.",
  inputSchema: z.object({ ref, text: z.string(), timeoutMs }),
  argv: (input) => ["type", input.ref, input.text, ...flag("timeout-ms", input.timeoutMs)],
});
