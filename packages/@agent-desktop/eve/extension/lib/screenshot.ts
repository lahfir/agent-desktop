import { randomUUID } from "node:crypto";

import { toolOutput, toolOutputPart, type ToolModelOutput } from "eve/tools";

import extension from "../extension";
import type { DesktopToolContext } from "./desktop";
import { DESKTOP_ROOT, quoteShellArg, type DesktopSandbox } from "./sandbox";

export interface ScreenshotOutput extends Record<string, unknown> {
  readonly path?: string;
  readonly width?: number;
  readonly height?: number;
  /** Data URL of the captured image, for channels/UIs to render inline and for the model. */
  readonly imageDataUrl?: string;
}

/** Structural subset of eve's sandbox handle. */
interface SandboxWithFiles extends DesktopSandbox {
  readBinaryFile?(options: { readonly path: string }): PromiseLike<Uint8Array | null>;
}

const MAX_INLINE_SCREENSHOT_BYTES = 3 * 1024 * 1024;

export function screenshotPath(): string {
  return `${DESKTOP_ROOT}/screenshots/${randomUUID()}.png`;
}

/**
 * Replaces the sandbox file with a data URL when inline screenshots are on.
 * The file is deleted once read, so screenshots don't pile up in the sandbox.
 * With inline screenshots off, the output keeps the path instead.
 */
export async function inlineScreenshot(data: unknown, ctx: DesktopToolContext): Promise<ScreenshotOutput> {
  const output = { ...(data as ScreenshotOutput | null) };
  if (!extension.config.inlineScreenshots || output.path === undefined) return output;
  const { path, ...rest } = output;
  try {
    const sandbox = (await ctx.getSandbox()) as SandboxWithFiles | null;
    if (sandbox?.readBinaryFile === undefined) return output;
    const bytes = await sandbox.readBinaryFile({ path });
    await sandbox.run({ command: `rm -f ${quoteShellArg(path)}` });
    if (!bytes || bytes.length === 0 || bytes.length > MAX_INLINE_SCREENSHOT_BYTES) return rest;
    return { ...rest, imageDataUrl: `data:image/png;base64,${Buffer.from(bytes).toString("base64")}` };
  } catch {
    // Inline rendering is best-effort; fall back to the path.
    return output;
  }
}

/** The model gets the pixels when they were captured inline, and never the sandbox path or the base64 as text. */
export function screenshotModelOutput(output: ScreenshotOutput): ToolModelOutput {
  const { imageDataUrl, path: _path, ...rest } = output;
  const summary = `Screenshot ${rest.width ?? "?"}x${rest.height ?? "?"} captured.`;
  if (imageDataUrl === undefined) return toolOutput.json({ ...rest, screenshot: summary });
  return toolOutput.content([
    toolOutputPart.text(summary),
    toolOutputPart.file(imageDataUrl.slice("data:image/png;base64,".length), { mediaType: "image/png" }),
  ]);
}
