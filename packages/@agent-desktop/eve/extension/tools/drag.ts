import { z } from "zod";

import { desktopTool, flag, ref } from "../lib/tool";

export default desktopTool({
  description: "Drag from one element or point to another. Requires the extension to run headed.",
  inputSchema: z.object({
    from: ref.optional(),
    fromXy: z.string().optional().describe("Source coordinates as x,y."),
    to: ref.optional(),
    toXy: z.string().optional().describe("Destination coordinates as x,y."),
  }),
  argv: (input) => [
    "drag",
    ...flag("from", input.from),
    ...flag("from-xy", input.fromXy),
    ...flag("to", input.to),
    ...flag("to-xy", input.toXy),
  ],
  deferred: true,
});
