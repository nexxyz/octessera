import {
  OLED_HEIGHT,
  OLED_WIDTH,
  type RuntimeOledFrameMessage,
  type OledFrame,
} from '@octessera/device-contracts';

export type OledFrameCacheFault =
  'malformed' | 'conflict' | 'missing' | 'future' | 'stale';

export type OledFrameCache = {
  acceptedRevision: number;
  acceptedPixels: Uint8Array | null;
  candidateRevision: number;
  candidatePixels: Uint8Array | null;
  fault: OledFrameCacheFault | null;
  readonly revision: number;
  readonly pixels: Uint8Array | null;
};

export function createOledFrameCache(): OledFrameCache {
  return {
    acceptedRevision: 0,
    acceptedPixels: null,
    candidateRevision: 0,
    candidatePixels: null,
    fault: null,
    get revision() {
      return this.acceptedRevision;
    },
    get pixels() {
      return this.acceptedPixels;
    },
  };
}

export function markOledFrameFault(
  cache: OledFrameCache,
  fault: OledFrameCacheFault,
): void {
  if (cache.fault !== 'conflict') cache.fault = fault;
}

export function acceptedOledFrame(cache: OledFrameCache): OledFrame {
  return {
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be',
    pixels:
      cache.acceptedPixels ?? new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2),
  };
}

export function ingestOledFrame(
  cache: OledFrameCache,
  message: RuntimeOledFrameMessage,
): void {
  const pixels = validateOledFrameMessage(message);
  if (pixels === null) {
    markOledFrameFault(cache, 'malformed');
    return;
  }
  if (message.revision <= cache.acceptedRevision) {
    handleAcceptedOledRevision(cache, message.revision, pixels);
    return;
  }
  if (message.revision === cache.candidateRevision) {
    handleCandidateOledRevision(cache, pixels);
    return;
  }
  if (message.revision < cache.candidateRevision) {
    markOledFrameFault(cache, 'stale');
    return;
  }
  cache.candidateRevision = message.revision;
  cache.candidatePixels = pixels;
}

export function acceptOledFrameReference(
  cache: OledFrameCache,
  revision: number | undefined,
): Uint8Array | null {
  if (revision === undefined) {
    markOledFrameFault(cache, 'missing');
    return cache.acceptedPixels;
  }
  if (!Number.isSafeInteger(revision) || revision < 1) {
    markOledFrameFault(cache, 'malformed');
    return cache.acceptedPixels;
  }
  if (revision === cache.acceptedRevision && revision > 0) {
    return cache.acceptedPixels;
  }
  if (revision === cache.candidateRevision && revision > 0) {
    if (!cache.candidatePixels) return cache.acceptedPixels;
    cache.acceptedRevision = revision;
    cache.acceptedPixels = cache.candidatePixels;
    cache.candidateRevision = 0;
    cache.candidatePixels = null;
    cache.fault = null;
    return cache.acceptedPixels;
  }
  markOledFrameFault(
    cache,
    revision > cache.acceptedRevision ? 'future' : 'stale',
  );
  return cache.acceptedPixels;
}

function decodeFramePixels(value: string): Uint8Array | null {
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(value) || value.length % 4 !== 0) {
    return null;
  }
  try {
    const decoded = atob(value);
    if (decoded.length !== OLED_WIDTH * OLED_HEIGHT * 2) return null;
    return Uint8Array.from(decoded, (char) => char.charCodeAt(0));
  } catch {
    return null;
  }
}

function validateOledFrameMessage(
  message: RuntimeOledFrameMessage,
): Uint8Array | null {
  if (typeof message.pixelsBase64 !== 'string') return null;
  const pixels = decodeFramePixels(message.pixelsBase64);
  if (
    message.width !== OLED_WIDTH ||
    message.height !== OLED_HEIGHT ||
    message.format !== 'rgb565be' ||
    !Number.isSafeInteger(message.revision) ||
    message.revision < 1 ||
    pixels === null
  ) {
    return null;
  }
  return pixels;
}

function handleAcceptedOledRevision(
  cache: OledFrameCache,
  revision: number,
  pixels: Uint8Array,
): void {
  if (
    revision === cache.acceptedRevision &&
    cache.acceptedPixels !== null &&
    !sameBytes(cache.acceptedPixels, pixels)
  ) {
    markOledFrameFault(cache, 'conflict');
  } else if (revision < cache.acceptedRevision) {
    markOledFrameFault(cache, 'stale');
  }
}

function handleCandidateOledRevision(
  cache: OledFrameCache,
  pixels: Uint8Array,
): void {
  if (!cache.candidatePixels) return;
  if (!sameBytes(cache.candidatePixels, pixels)) {
    markOledFrameFault(cache, 'conflict');
    cache.candidatePixels = null;
  }
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return (
    left.length === right.length &&
    left.every((byte, index) => byte === right[index])
  );
}
