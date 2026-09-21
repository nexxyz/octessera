export const RUNTIME_FX_PARAM_IDS = [
  "rateHz",
  "depthPct",
  "feedback",
  "mixPct",
  "spreadPct",
  "centerHz",
  "q",
  "decay",
  "damp",
  "chancePct",
  "sliceMs",
  "threshold",
  "amountPct",
  "attackMs",
  "releaseMs",
  "drive",
  "clip",
  "bits",
  "rateDiv",
  "thresholdDb",
  "ratio",
  "makeupDb",
  "lowGainDb",
  "midGainDb",
  "midFreqHz",
  "midQ",
  "highGainDb",
  "saturationPct",
  "cracklePct",
  "warpDepthPct",
] as const;
export type RuntimeFxParamId = (typeof RUNTIME_FX_PARAM_IDS)[number];

export type RuntimeMomentaryFxTarget =
  | { type: "global" }
  | { type: "fx_bus"; index: number }
  | { type: "instrument"; index: number };

export type RuntimeAudioCommand =
  | {
      type: "set_audio_config";
      revision: number;
      requestId?: string;
      generation: number;
      config: Record<string, unknown>;
    }
  | {
      type: "set_dsp_config";
      generation: number;
      config: Record<string, unknown>;
    }
  | { type: "set_master_volume"; generation: number; volumePct: number }
  | {
      type: "set_instrument_mixer";
      instrumentSlot: number;
      generation: number;
      volumePct?: number;
      panPos?: number;
    }
  | {
      type: "set_instrument_slot";
      instrumentSlot: number;
      generation: number;
      config: Record<string, unknown>;
    }
  | {
      type: "set_fx_bus_mixer";
      busIndex: number;
      generation: number;
      panPos?: number;
      volumePct?: number;
    }
  | {
      type: "set_synth_param";
      instrumentSlot: number;
      generation: number;
      path: string;
      value: number;
    }
  | {
      type: "set_sample_bank_param";
      instrumentSlot: number;
      generation: number;
      path: string;
      value: number;
    }
  | {
      type: "set_fx_bus_param";
      busIndex: number;
      slotIndex: number;
      generation: number;
      param: RuntimeFxParamId;
      value: number;
    }
  | {
      type: "set_fx_bus_slot";
      busIndex: number;
      slotIndex: number;
      generation: number;
      fxType: string;
      params: Record<string, unknown>;
    }
  | {
      type: "set_global_fx_slot";
      slotIndex: number;
      generation: number;
      fxType: string;
      params: Record<string, unknown>;
    }
  | {
      type: "set_global_fx_param";
      slotIndex: number;
      generation: number;
      param: RuntimeFxParamId;
      value: number;
    }
  | {
      type: "momentary_fx_start";
      id: string;
      epoch: number;
      fxType: string;
      params: Record<string, unknown>;
      target: RuntimeMomentaryFxTarget;
    }
  | {
      type: "momentary_fx_update";
      id: string;
      epoch: number;
      params: Record<string, unknown>;
    }
  | {
      type: "momentary_fx_stop";
      id: string;
      epoch: number;
    }
  | {
      type: "sample_preview";
      instrumentSlot: number;
      sampleSlot: number;
      path: string;
      velocity: number;
    };

type RequireRuntimeAudioCommandField<
  T extends RuntimeAudioCommand,
  K extends keyof T,
> = Omit<T, K> & Required<Pick<T, K>>;

export type RuntimeAudioCommandOutbound =
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_audio_config" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_dsp_config" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_master_volume" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_instrument_mixer" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_instrument_slot" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_fx_bus_mixer" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_synth_param" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_sample_bank_param" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_fx_bus_param" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_fx_bus_slot" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_global_fx_slot" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "set_global_fx_param" }>,
      "generation"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "momentary_fx_start" }>,
      "epoch"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "momentary_fx_update" }>,
      "epoch"
    >
  | RequireRuntimeAudioCommandField<
      Extract<RuntimeAudioCommand, { type: "momentary_fx_stop" }>,
      "epoch"
    >
  | Extract<RuntimeAudioCommand, { type: "sample_preview" }>;
