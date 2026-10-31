import { z } from "zod";

import { desktopTool, flag, ref } from "../lib/tool";

export default desktopTool({
  description: "Move the pointer over an element or to x,y screen coordinates. Requires the extension to run headed.",
  inputSchema: z.object({ ref: ref.optional(), xy: z.string().optional().describe("Screen coordinates as x,y.") }),
  argv: (input) => ["hover", ...(input.ref ? [input.ref] : []), ...flag("xy", input.xy)],
  deferred: true,
});
