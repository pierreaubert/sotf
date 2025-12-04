// Export utility for optimized EQ parameters in various formats

import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

export type ExportFormat = "apo" | "aupreset" | "rme" | "rme-room";

/**
 * Export optimized EQ parameters in selected format
 */
export async function exportEQ(
  filterParams: number[],
  sampleRate: number,
  peqModel: string,
  lossType: string | null,
  speakerName: string | null,
  format: ExportFormat,
): Promise<void> {
  try {
    // Generate content based on format
    let content: string;
    switch (format) {
      case "apo":
        content = (await invoke("generate_apo_format", {
          filterParams,
          sampleRate,
          peqModel,
        })) as string;
        break;
      case "aupreset": {
        const presetName = generatePresetName(lossType, speakerName);
        content = (await invoke("generate_aupreset_format", {
          filterParams,
          sampleRate,
          peqModel,
          presetName,
        })) as string;
        break;
      }
      case "rme":
        content = (await invoke("generate_rme_format", {
          filterParams,
          sampleRate,
          peqModel,
        })) as string;
        break;
      case "rme-room":
        content = (await invoke("generate_rme_room_format", {
          filterParams,
          sampleRate,
          peqModel,
        })) as string;
        break;
      default:
        throw new Error(`Unsupported export format: ${format}`);
    }

    // Generate default filename with loss type and speaker name
    const defaultFilename = generateFilename(lossType, speakerName, format);

    // Get file filters based on format
    const filters = getFileFilters(format);

    // Show native save dialog
    const filePath = await save({
      defaultPath: defaultFilename,
      filters,
    });

    if (!filePath) {
      return;
    }

    // Write the file using Tauri's filesystem API
    await writeTextFile(filePath, content);
  } catch (error) {
    console.error(`[EXPORT] Export failed:`, error);
    throw error;
  }
}

/**
 * Generate preset name for AUpreset format
 */
function generatePresetName(
  lossType: string | null,
  speakerName: string | null,
): string {
  // Extract loss type
  let lossTypePart = "flat";
  if (lossType) {
    if (lossType.includes("score")) {
      lossTypePart = "score";
    } else if (lossType.includes("flat")) {
      lossTypePart = "flat";
    }
  }

  // Use speaker name or "Unknown"
  const speakerPart =
    speakerName && speakerName.trim() !== "" ? speakerName.trim() : "Unknown";

  return `AutoEQ ${lossTypePart} - ${speakerPart}`;
}

/**
 * Generate filename with loss type, speaker name, and timestamp
 * Format: autoeq-iir-{flat|score}-{speaker-name}-YYYY-MM-DD.{ext}
 */
function generateFilename(
  lossType: string | null,
  speakerName: string | null,
  format: ExportFormat,
): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");

  // Extract loss type (flat or score)
  let lossTypePart = "flat";
  if (lossType) {
    // Extract the loss type from patterns like "speaker-flat", "headphone-score", etc.
    if (lossType.includes("score")) {
      lossTypePart = "score";
    } else if (lossType.includes("flat")) {
      lossTypePart = "flat";
    }
  }

  // Sanitize speaker name for filename
  let speakerPart = "unknown";
  if (speakerName && speakerName.trim() !== "") {
    speakerPart = speakerName
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-") // Replace non-alphanumeric with hyphens
      .replace(/^-+|-+$/g, ""); // Remove leading/trailing hyphens
  }

  // Get file extension based on format
  const extension = getFileExtension(format);

  return `autoeq-iir-${lossTypePart}-${speakerPart}-${year}-${month}-${day}.${extension}`;
}

/**
 * Get file extension for the export format
 */
function getFileExtension(format: ExportFormat): string {
  switch (format) {
    case "apo":
      return "txt";
    case "aupreset":
      return "aupreset";
    case "rme":
      return "tmeq";
    case "rme-room":
      return "tmreq";
    default:
      return "txt";
  }
}

/**
 * Get file filters for save dialog based on format
 */
function getFileFilters(
  format: ExportFormat,
): Array<{ name: string; extensions: string[] }> {
  switch (format) {
    case "apo":
      return [
        { name: "Text Files", extensions: ["txt"] },
        { name: "All Files", extensions: ["*"] },
      ];
    case "aupreset":
      return [
        { name: "AUpreset Files", extensions: ["aupreset"] },
        { name: "All Files", extensions: ["*"] },
      ];
    case "rme":
      return [
        { name: "TotalMix Channel EQ", extensions: ["tmeq"] },
        { name: "All Files", extensions: ["*"] },
      ];
    case "rme-room":
      return [
        { name: "TotalMix Room EQ", extensions: ["tmreq"] },
        { name: "All Files", extensions: ["*"] },
      ];
    default:
      return [{ name: "All Files", extensions: ["*"] }];
  }
}
