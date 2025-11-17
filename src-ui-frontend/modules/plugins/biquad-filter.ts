// Browser-based Biquad Filter Implementation
// Based on audio EQ cookbook: https://webaudio.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html

export type FilterType =
  | "Peak"
  | "Lowshelf"
  | "Highshelf"
  | "Lowpass"
  | "Highpass"
  | "Bandpass"
  | "Notch";

export interface BiquadCoefficients {
  b0: number;
  b1: number;
  b2: number;
  a0: number;
  a1: number;
  a2: number;
}

export class BiquadFilter {
  public filterType: FilterType;
  public frequency: number;
  public sampleRate: number;
  public q: number;
  public gainDb: number;
  private coefficients: BiquadCoefficients;

  constructor(
    filterType: FilterType,
    frequency: number,
    sampleRate: number,
    q: number,
    gainDb: number,
  ) {
    this.filterType = filterType;
    this.frequency = frequency;
    this.sampleRate = sampleRate;
    this.q = q;
    this.gainDb = gainDb;
    this.coefficients = this.calculateCoefficients();
  }

  /**
   * Calculate biquad filter coefficients based on filter type
   */
  private calculateCoefficients(): BiquadCoefficients {
    const w0 = (2 * Math.PI * this.frequency) / this.sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const alpha = sinW0 / (2 * this.q);
    const A = Math.pow(10, this.gainDb / 40); // sqrt of gain in linear scale

    let b0 = 0,
      b1 = 0,
      b2 = 0,
      a0 = 1,
      a1 = 0,
      a2 = 0;

    switch (this.filterType) {
      case "Peak": {
        b0 = 1 + alpha * A;
        b1 = -2 * cosW0;
        b2 = 1 - alpha * A;
        a0 = 1 + alpha / A;
        a1 = -2 * cosW0;
        a2 = 1 - alpha / A;
        break;
      }

      case "Lowshelf": {
        const sqrtA = Math.sqrt(A);
        b0 = A * (A + 1 - (A - 1) * cosW0 + 2 * sqrtA * alpha);
        b1 = 2 * A * (A - 1 - (A + 1) * cosW0);
        b2 = A * (A + 1 - (A - 1) * cosW0 - 2 * sqrtA * alpha);
        a0 = A + 1 + (A - 1) * cosW0 + 2 * sqrtA * alpha;
        a1 = -2 * (A - 1 + (A + 1) * cosW0);
        a2 = A + 1 + (A - 1) * cosW0 - 2 * sqrtA * alpha;
        break;
      }

      case "Highshelf": {
        const sqrtA = Math.sqrt(A);
        b0 = A * (A + 1 + (A - 1) * cosW0 + 2 * sqrtA * alpha);
        b1 = -2 * A * (A - 1 + (A + 1) * cosW0);
        b2 = A * (A + 1 + (A - 1) * cosW0 - 2 * sqrtA * alpha);
        a0 = A + 1 - (A - 1) * cosW0 + 2 * sqrtA * alpha;
        a1 = 2 * (A - 1 - (A + 1) * cosW0);
        a2 = A + 1 - (A - 1) * cosW0 - 2 * sqrtA * alpha;
        break;
      }

      case "Lowpass": {
        b0 = (1 - cosW0) / 2;
        b1 = 1 - cosW0;
        b2 = (1 - cosW0) / 2;
        a0 = 1 + alpha;
        a1 = -2 * cosW0;
        a2 = 1 - alpha;
        break;
      }

      case "Highpass": {
        b0 = (1 + cosW0) / 2;
        b1 = -(1 + cosW0);
        b2 = (1 + cosW0) / 2;
        a0 = 1 + alpha;
        a1 = -2 * cosW0;
        a2 = 1 - alpha;
        break;
      }

      case "Bandpass": {
        b0 = alpha;
        b1 = 0;
        b2 = -alpha;
        a0 = 1 + alpha;
        a1 = -2 * cosW0;
        a2 = 1 - alpha;
        break;
      }

      case "Notch": {
        b0 = 1;
        b1 = -2 * cosW0;
        b2 = 1;
        a0 = 1 + alpha;
        a1 = -2 * cosW0;
        a2 = 1 - alpha;
        break;
      }
    }

    return { b0, b1, b2, a0, a1, a2 };
  }

  /**
   * Compute frequency response (magnitude in dB) at given frequencies
   */
  public computeFrequencyResponse(frequencies: number[]): number[] {
    const { b0, b1, b2, a0, a1, a2 } = this.coefficients;
    const magnitudesDb: number[] = [];

    for (const freq of frequencies) {
      const w = (2 * Math.PI * freq) / this.sampleRate;
      const cosW = Math.cos(w);
      const sinW = Math.sin(w);

      // Complex numerator: b0 + b1*z^-1 + b2*z^-2
      const numReal = b0 + b1 * cosW + b2 * Math.cos(2 * w);
      const numImag = -b1 * sinW - b2 * Math.sin(2 * w);

      // Complex denominator: a0 + a1*z^-1 + a2*z^-2
      const denReal = a0 + a1 * cosW + a2 * Math.cos(2 * w);
      const denImag = -a1 * sinW - a2 * Math.sin(2 * w);

      // Complex division: |H(w)| = |num| / |den|
      const numMag = Math.sqrt(numReal * numReal + numImag * numImag);
      const denMag = Math.sqrt(denReal * denReal + denImag * denImag);

      // Convert to dB
      const magnitudeDb = 20 * Math.log10(numMag / denMag);
      magnitudesDb.push(magnitudeDb);
    }

    return magnitudesDb;
  }

  /**
   * Generate logarithmically spaced frequencies
   */
  static generateLogFrequencies(
    minFreq: number,
    maxFreq: number,
    numPoints: number,
  ): number[] {
    const logMin = Math.log10(minFreq);
    const logMax = Math.log10(maxFreq);
    const frequencies: number[] = [];

    for (let i = 0; i < numPoints; i++) {
      const logFreq = logMin + (logMax - logMin) * (i / (numPoints - 1));
      frequencies.push(Math.pow(10, logFreq));
    }

    return frequencies;
  }
}
