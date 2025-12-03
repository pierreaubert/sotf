// Speaker Configuration Definitions
// Standard speaker positions based on ITU-R BS.775 and Dolby Atmos specs
// Azimuth: 0° = front, +90° = left, -90° = right, ±180° = back
// Elevation: 0° = ear level, +90° = overhead

/**
 * Speaker position in 3D space using spherical coordinates
 */
export interface SpeakerPosition {
  label: string; // Channel label (e.g., "FL", "C", "TFL")
  name: string; // Full name (e.g., "Front Left", "Center")
  azimuth: number; // Horizontal angle in degrees (-180 to +180)
  elevation: number; // Vertical angle in degrees (0 to 90)
  channel: number; // Channel index in output array
  isLFE: boolean; // True if this is the LFE channel
}

/**
 * Speaker configuration preset
 */
export interface SpeakerConfig {
  id: string; // Configuration ID (e.g., "5.1", "7.1.4")
  name: string; // Display name
  description: string;
  totalChannels: number; // Total number of channels including LFE
  speakers: SpeakerPosition[];
  channelOrder: string[]; // Standard channel ordering (e.g., ["FL", "FR", "C", "LFE", ...])
}

/**
 * Standard speaker configurations
 */
export const SPEAKER_CONFIGS: Record<string, SpeakerConfig> = {
  "5.1": {
    id: "5.1",
    name: "5.1 Surround",
    description: "Standard 5.1 surround sound (ITU-R BS.775)",
    totalChannels: 6,
    channelOrder: ["FL", "FR", "C", "LFE", "SL", "SR"],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 110,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -110,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
    ],
  },

  "7.1": {
    id: "7.1",
    name: "7.1 Surround",
    description: "7.1 surround with side and back speakers",
    totalChannels: 8,
    channelOrder: ["FL", "FR", "C", "LFE", "SL", "SR", "BL", "BR"],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 90,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -90,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "BL",
        name: "Back Left",
        azimuth: 150,
        elevation: 0,
        channel: 6,
        isLFE: false,
      },
      {
        label: "BR",
        name: "Back Right",
        azimuth: -150,
        elevation: 0,
        channel: 7,
        isLFE: false,
      },
    ],
  },

  "5.1.2": {
    id: "5.1.2",
    name: "5.1.2 Atmos",
    description: "5.1 with 2 height speakers",
    totalChannels: 8,
    channelOrder: ["FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR"],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 110,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -110,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 6,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 7,
        isLFE: false,
      },
    ],
  },

  "5.1.4": {
    id: "5.1.4",
    name: "5.1.4 Atmos",
    description: "5.1 with 4 height speakers",
    totalChannels: 10,
    channelOrder: ["FL", "FR", "C", "LFE", "SL", "SR", "TFL", "TFR", "TBL", "TBR"],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 110,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -110,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 6,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 7,
        isLFE: false,
      },
      {
        label: "TBL",
        name: "Top Back Left",
        azimuth: 150,
        elevation: 45,
        channel: 8,
        isLFE: false,
      },
      {
        label: "TBR",
        name: "Top Back Right",
        azimuth: -150,
        elevation: 45,
        channel: 9,
        isLFE: false,
      },
    ],
  },

  "7.1.2": {
    id: "7.1.2",
    name: "7.1.2 Atmos",
    description: "7.1 with 2 height speakers",
    totalChannels: 10,
    channelOrder: [
      "FL",
      "FR",
      "C",
      "LFE",
      "SL",
      "SR",
      "BL",
      "BR",
      "TFL",
      "TFR",
    ],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 90,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -90,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "BL",
        name: "Back Left",
        azimuth: 150,
        elevation: 0,
        channel: 6,
        isLFE: false,
      },
      {
        label: "BR",
        name: "Back Right",
        azimuth: -150,
        elevation: 0,
        channel: 7,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 8,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 9,
        isLFE: false,
      },
    ],
  },

  "7.1.4": {
    id: "7.1.4",
    name: "7.1.4 Atmos",
    description: "7.1 with 4 height speakers",
    totalChannels: 12,
    channelOrder: [
      "FL",
      "FR",
      "C",
      "LFE",
      "SL",
      "SR",
      "BL",
      "BR",
      "TFL",
      "TFR",
      "TBL",
      "TBR",
    ],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 90,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -90,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "BL",
        name: "Back Left",
        azimuth: 150,
        elevation: 0,
        channel: 6,
        isLFE: false,
      },
      {
        label: "BR",
        name: "Back Right",
        azimuth: -150,
        elevation: 0,
        channel: 7,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 8,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 9,
        isLFE: false,
      },
      {
        label: "TBL",
        name: "Top Back Left",
        azimuth: 150,
        elevation: 45,
        channel: 10,
        isLFE: false,
      },
      {
        label: "TBR",
        name: "Top Back Right",
        azimuth: -150,
        elevation: 45,
        channel: 11,
        isLFE: false,
      },
    ],
  },

  "9.1.4": {
    id: "9.1.4",
    name: "9.1.4 Atmos",
    description: "9.1 with 4 height speakers (adds wide channels)",
    totalChannels: 14,
    channelOrder: [
      "FL",
      "FR",
      "C",
      "LFE",
      "SL",
      "SR",
      "BL",
      "BR",
      "WL",
      "WR",
      "TFL",
      "TFR",
      "TBL",
      "TBR",
    ],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 90,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -90,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "BL",
        name: "Back Left",
        azimuth: 150,
        elevation: 0,
        channel: 6,
        isLFE: false,
      },
      {
        label: "BR",
        name: "Back Right",
        azimuth: -150,
        elevation: 0,
        channel: 7,
        isLFE: false,
      },
      {
        label: "WL",
        name: "Wide Left",
        azimuth: 60,
        elevation: 0,
        channel: 8,
        isLFE: false,
      },
      {
        label: "WR",
        name: "Wide Right",
        azimuth: -60,
        elevation: 0,
        channel: 9,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 10,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 11,
        isLFE: false,
      },
      {
        label: "TBL",
        name: "Top Back Left",
        azimuth: 150,
        elevation: 45,
        channel: 12,
        isLFE: false,
      },
      {
        label: "TBR",
        name: "Top Back Right",
        azimuth: -150,
        elevation: 45,
        channel: 13,
        isLFE: false,
      },
    ],
  },

  "9.1.6": {
    id: "9.1.6",
    name: "9.1.6 Atmos",
    description: "9.1 with 6 height speakers (adds top mid channels)",
    totalChannels: 16,
    channelOrder: [
      "FL",
      "FR",
      "C",
      "LFE",
      "SL",
      "SR",
      "BL",
      "BR",
      "WL",
      "WR",
      "TFL",
      "TFR",
      "TBL",
      "TBR",
      "TMiL",
      "TMiR",
    ],
    speakers: [
      {
        label: "FL",
        name: "Front Left",
        azimuth: 30,
        elevation: 0,
        channel: 0,
        isLFE: false,
      },
      {
        label: "FR",
        name: "Front Right",
        azimuth: -30,
        elevation: 0,
        channel: 1,
        isLFE: false,
      },
      {
        label: "C",
        name: "Center",
        azimuth: 0,
        elevation: 0,
        channel: 2,
        isLFE: false,
      },
      {
        label: "LFE",
        name: "Low Frequency Effects",
        azimuth: 0,
        elevation: 0,
        channel: 3,
        isLFE: true,
      },
      {
        label: "SL",
        name: "Side Left",
        azimuth: 90,
        elevation: 0,
        channel: 4,
        isLFE: false,
      },
      {
        label: "SR",
        name: "Side Right",
        azimuth: -90,
        elevation: 0,
        channel: 5,
        isLFE: false,
      },
      {
        label: "BL",
        name: "Back Left",
        azimuth: 150,
        elevation: 0,
        channel: 6,
        isLFE: false,
      },
      {
        label: "BR",
        name: "Back Right",
        azimuth: -150,
        elevation: 0,
        channel: 7,
        isLFE: false,
      },
      {
        label: "WL",
        name: "Wide Left",
        azimuth: 60,
        elevation: 0,
        channel: 8,
        isLFE: false,
      },
      {
        label: "WR",
        name: "Wide Right",
        azimuth: -60,
        elevation: 0,
        channel: 9,
        isLFE: false,
      },
      {
        label: "TFL",
        name: "Top Front Left",
        azimuth: 30,
        elevation: 45,
        channel: 10,
        isLFE: false,
      },
      {
        label: "TFR",
        name: "Top Front Right",
        azimuth: -30,
        elevation: 45,
        channel: 11,
        isLFE: false,
      },
      {
        label: "TBL",
        name: "Top Back Left",
        azimuth: 150,
        elevation: 45,
        channel: 12,
        isLFE: false,
      },
      {
        label: "TBR",
        name: "Top Back Right",
        azimuth: -150,
        elevation: 45,
        channel: 13,
        isLFE: false,
      },
      {
        label: "TMiL",
        name: "Top Middle Left",
        azimuth: 90,
        elevation: 45,
        channel: 14,
        isLFE: false,
      },
      {
        label: "TMiR",
        name: "Top Middle Right",
        azimuth: -90,
        elevation: 45,
        channel: 15,
        isLFE: false,
      },
    ],
  },
};

/**
 * Get channel configuration by ID
 */
export function getSpeakerConfig(id: string): SpeakerConfig | undefined {
  return SPEAKER_CONFIGS[id];
}

/**
 * Get all available configuration IDs
 */
export function getAvailableConfigs(): string[] {
  return Object.keys(SPEAKER_CONFIGS);
}

/**
 * Calculate panning gain for a speaker based on source position
 * Uses Vector Base Amplitude Panning (VBAP) principles
 *
 * @param sourceAzimuth - Source azimuth in degrees
 * @param sourceElevation - Source elevation in degrees
 * @param speakerAzimuth - Speaker azimuth in degrees
 * @param speakerElevation - Speaker elevation in degrees
 * @returns Gain value (0.0 to 1.0)
 */
export function calculatePanningGain(
  sourceAzimuth: number,
  sourceElevation: number,
  speakerAzimuth: number,
  speakerElevation: number,
): number {
  // Convert to radians
  const srcAz = (sourceAzimuth * Math.PI) / 180;
  const srcEl = (sourceElevation * Math.PI) / 180;
  const spkAz = (speakerAzimuth * Math.PI) / 180;
  const spkEl = (speakerElevation * Math.PI) / 180;

  // Convert spherical to Cartesian coordinates
  const srcX = Math.cos(srcEl) * Math.sin(srcAz);
  const srcY = Math.cos(srcEl) * Math.cos(srcAz);
  const srcZ = Math.sin(srcEl);

  const spkX = Math.cos(spkEl) * Math.sin(spkAz);
  const spkY = Math.cos(spkEl) * Math.cos(spkAz);
  const spkZ = Math.sin(spkEl);

  // Calculate dot product (cosine of angle between vectors)
  const dotProduct = srcX * spkX + srcY * spkY + srcZ * spkZ;

  // Map from [-1, 1] to [0, 1] with cosine law
  // Use raised cosine for smoother panning
  const gain = Math.max(0, dotProduct);

  return gain;
}

/**
 * Calculate all speaker gains for a stereo source
 * Returns an array of gains for each speaker
 */
export function calculateStereoUpmixGains(
  config: SpeakerConfig,
  leftAzimuth: number = 30, // Default stereo left position
  rightAzimuth: number = -30, // Default stereo right position
): { left: number[]; right: number[] } {
  const leftGains: number[] = [];
  const rightGains: number[] = [];

  for (const speaker of config.speakers) {
    if (speaker.isLFE) {
      // LFE gets equal mix of both channels
      leftGains.push(0.5);
      rightGains.push(0.5);
    } else {
      const leftGain = calculatePanningGain(
        leftAzimuth,
        0,
        speaker.azimuth,
        speaker.elevation,
      );
      const rightGain = calculatePanningGain(
        rightAzimuth,
        0,
        speaker.azimuth,
        speaker.elevation,
      );

      leftGains.push(leftGain);
      rightGains.push(rightGain);
    }
  }

  // Normalize gains to prevent clipping
  const maxGain = Math.max(
    ...leftGains.map((g, i) => g + rightGains[i]),
  );
  if (maxGain > 1.0) {
    const scale = 1.0 / maxGain;
    for (let i = 0; i < leftGains.length; i++) {
      leftGains[i] *= scale;
      rightGains[i] *= scale;
    }
  }

  return { left: leftGains, right: rightGains };
}
