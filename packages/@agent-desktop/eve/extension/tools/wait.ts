import { z } from "zod";

import { desktopTool, flag } from "../lib/tool";

export default desktopTool({
  description:
    "Wait for a window, text, element, menu, or a fixed time. Prefer a condition over a fixed time; set exactly one of window, text, element, menu, menuClosed, or ms.",
  inputSchema: z.object({
    window: z.string().optional().describe("Wait until a window with this title (or part of it) appears."),
    text: z.string().optional().describe("Wait until this text appears in the app's tree."),
    element: z.string().optional().describe("Wait until this ref exists, or meets predicate."),
    predicate: z.enum(["exists", "enabled", "visible", "actionable", "value"]).optional(),
    value: z.string().optional().describe("Expected value for predicate value."),
    menu: z.boolean().optional().describe("Wait until a menu is open."),
    menuClosed: z.boolean().optional().describe("Wait until the open menu is dismissed."),
    ms: z.number().int().positive().max(60_000).optional().describe("Fixed pause in milliseconds."),
    app: z.string().optional().describe("Scope the wait to this app."),
    timeout: z.number().int().positive().max(120_000).optional().describe("Timeout for condition waits (default 30000)."),
  }),
  argv: (input) => [
    "wait",
    ...(input.ms !== undefined ? [String(input.ms)] : []),
    ...flag("window", input.window),
    ...flag("text", input.text),
    ...flag("element", input.element),
    ...flag("predicate", input.predicate),
    ...flag("value", input.value),
    ...flag("menu", input.menu),
    ...flag("menu-closed", input.menuClosed),
    ...flag("app", input.app),
    ...flag("timeout", input.timeout),
  ],
});
