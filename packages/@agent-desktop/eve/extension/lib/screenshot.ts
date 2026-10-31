import { randomUUID } from "node:crypto";

import { toolOutput, toolOutputPart, type ToolModelOutput } from "eve/tools";

import extension from "../extension";
import type { DesktopToolContext } from "./desktop";
import { DESKTOP_ROOT } from "./sandbox";

export interface ScreenshotOutput extends Record<string, unknown> {
  readonly path?: string;
  readonly width?: number;
  readonly height?: number;
  /** Data URL of the captured image, for channels/UIs to render inline. */
  readonly imageDataUrl?: string;
}

/** Structural subset of eve's sandbox handle. */
interface SandboxWithFiles {
  readBinaryFile?(options: { readonly path: string }): PromiseLike<Uint8Array | null>;
}

const MAX_INLINE_SCREENSHOT_BYTES = 3 * 1024 * 1024;

export function screenshotPath(): string {
  return `${DESKTOP_ROOT}/screenshots/${randomUUID()}.png`;
}

/** Adds the captured PNG as a data URL when inline screenshots are enabled. */
export async function inlineScreenshot(data: unknown, ctx: DesktopToolContext): Promise<ScreenshotOutput> {
  const output = { ...(data as ScreenshotOutput | null) };
  const config = extension.config;
  if ((!config.inlineScreenshots && !config.screenshotsToModel) || output.path === undefined) return output;
  try {
    const sandbox = (await ctx.getSandbox()) as SandboxWithFiles | null;
    const bytes = await sandbox?.readBinaryFile?.({ path: output.path });
    if (!bytes || bytes.length === 0 || bytes.length > MAX_INLINE_SCREENSHOT_BYTES) return output;
    return { ...output, imageDataUrl: `data:image/png;base64,${Buffer.from(bytes).toString("base64")}` };
  } catch {
    // Inline rendering is best-effort; the path in the output always works.
    return output;
  }
}

/** The model gets the pixels (when enabled) but never the sandbox path or the base64 as text. */
export function screenshotModelOutput(output: ScreenshotOutput): ToolModelOutput {
  const { imageDataUrl, path: _path, ...rest } = output;
  const summary = `Screenshot ${rest.width ?? "?"}x${rest.height ?? "?"} captured.`;
  if (imageDataUrl === undefined || !extension.config.screenshotsToModel) {
    return toolOutput.json({ ...rest, screenshot: imageDataUrl ? "Captured and shown to the user." : summary });
  }
  return toolOutput.content([
    toolOutputPart.text(summary),
    toolOutputPart.file(imageDataUrl.slice("data:image/png;base64,".length), { mediaType: "image/png" }),
  ]);
}
