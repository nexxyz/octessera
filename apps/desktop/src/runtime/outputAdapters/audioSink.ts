import type { MusicalEvent } from "@cellsymphony/musical-events";
import { nativeAudioBridge } from "../../audio/nativeAudioBridge";

export async function sendEventsToAudio(events: MusicalEvent[], masterVolume: number): Promise<void> {
  void masterVolume;
  for (const event of events) {
    await nativeAudioBridge.trigger(event);
  }
}
