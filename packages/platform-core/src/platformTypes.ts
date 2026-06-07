import type { MappingConfig } from "@cellsymphony/mapping-core";
import type { TransportFrame } from "@cellsymphony/device-contracts";

export type ScanMode = "immediate" | "scanning";
export type ScanAxis = "rows" | "columns";
export type Direction = "forward" | "reverse";
export type NoteUnit = "1/16" | "1/8" | "1/4" | "1/2" | "1/1";
export type SectionCount = "1" | "2" | "4" | "8";
export type Curve = "linear" | "curve";
export type VoiceStealingMode = "off" | "lenient" | "balanced" | "aggressive";
export type NumericDisplayMode = "bar" | "numbers" | "bar+numbers";
export type DanceMode = "none" | "mix" | "pan" | "fx" | "trigger-gate" | "xy";
export type MomentaryFxType = "none" | "stutter" | "freeze" | "filter_sweep" | "pitch_shift";
export type MomentaryFxTarget =
  | { type: "global" }
  | { type: "fx_bus"; index: number }
  | { type: "instrument"; index: number };
export type BarValue = { frac: number; numChars: number; style?: "fill" | "marker" };
export type ScaleId = "chromatic" | "major" | "natural_minor" | "dorian" | "mixolydian" | "major_pentatonic" | "minor_pentatonic" | "harmonic_minor";
export type RootName = "C" | "C#" | "D" | "D#" | "E" | "F" | "F#" | "G" | "G#" | "A" | "A#" | "B";
export type ParamModAxis = "x" | "y";
export type ParamModSlotBinding = AuxTurnBinding & { invert: boolean };
export type ParamModAxisSlots = { x: [ParamModSlotBinding | null, ParamModSlotBinding | null]; y: [ParamModSlotBinding | null, ParamModSlotBinding | null] };
type OutOfRangeMode = "clamp" | "wrap";
type PitchSettings = { startingNote: number; lowestNote: number; highestNote: number; outOfRange: OutOfRangeMode; scale: ScaleId; root: RootName };
type PitchLaneConfig = { enabled: boolean; steps: number; restartEachSection: boolean };
export type ValueLaneConfig = { enabled: boolean; from: number; to: number; gridOffset: number; curve: Curve };
type AxisModConfig = { pitch: PitchLaneConfig; velocity: ValueLaneConfig; filterCutoff: ValueLaneConfig; filterResonance: ValueLaneConfig };
type TriggerAction = "none" | "note_on" | "note_off";
export type TriggerProbabilityMode = "zero" | "custom" | "full";
export type TriggerProbabilityCellState = "zero" | "low" | "high" | "full";

export type EnvConfig = { attackMs: number; decayMs: number; sustainPct: number; releaseMs: number };

export type OscConfig = {
  waveform: "sine" | "saw" | "square" | "pulse" | "triangle";
  levelPct: number;
  octave: -2 | -1 | 0 | 1 | 2;
  detuneCents: number;
  pulseWidthPct: number;
};

export type FilterConfig = {
  type: "lowpass" | "highpass" | "bandpass" | "notch";
  cutoffHz: number;
  resonance: number;
  envAmountPct: number;
  keyTrackingPct: number;
};

export type AmpConfig = { gainPct: number; velocitySensitivityPct: number };

export type SynthConfig = {
  osc1: OscConfig;
  osc2: OscConfig;
  amp: AmpConfig;
  ampEnv: EnvConfig;
  filter: FilterConfig;
  filterEnv: EnvConfig;
};

export type InstrumentSlotConfig = {
  type: "synth" | "sampler" | "midi" | "none";
  autoName: boolean;
  name: string;
  noteBehavior: "oneshot" | "hold";
  midi: { enabled: boolean; channel: number };
  synth: SynthConfig;
  sample: {
    baseVelocity: number;
    velocityLevelsEnabled: boolean;
    velocityLevels: { high: number; medium: number; low: number };
    selectedSlot: number;
    slots: Array<{ path: string | null }>;
    tuneSemis: number;
    amp: AmpConfig;
    ampEnv: EnvConfig;
    filter: FilterConfig;
    filterEnv: EnvConfig;
    assignments: Array<{ x: number; y: number; sampleSlot: number; level?: "high" | "medium" | "low" }>;
  };
  midiEngine: {
    velocity: number;
    durationMs: number;
  };
  mixer?: { route: string; panPos: number; volume: number };
};

export type FxBusEffectType =
  | "none"
  | "reverb"
  | "delay"
  | "tremolo"
  | "vibrato"
  | "auto_pan"
  | "chorus"
  | "flanger"
  | "wah"
  | "filter_lfo"
  | "duck"
  | "bitcrusher"
  | "saturator"
  | "distortion"
  | "glitch"
  | "compressor"
  | "eq";

export type GlobalFxEffectType =
  | "none"
  | "vinyl"
  | "eq"
  | "compressor"
  | "saturator"
  | "distortion";

export type FxSlotConfig<TType extends string> = {
  type: TType;
  params: Record<string, unknown>;
};

export type FxBusSlotConfig = FxSlotConfig<FxBusEffectType>;
export type GlobalFxSlotConfig = FxSlotConfig<GlobalFxEffectType>;

export type FxBusConfig = {
  slot1: FxBusSlotConfig;
  slot2: FxBusSlotConfig;
  panPos: number;
  autoName: boolean;
  name: string;
};

export type MasterFxConfig = {
  slots: GlobalFxSlotConfig[];
};

export type MixerConfig = {
  buses: FxBusConfig[];
  master: MasterFxConfig;
};

export type MomentaryFxConfig = { fxType: MomentaryFxType; params: Record<string, unknown>; targetKey: string };
export type FxCellConfig = { x: number; y: number; config: MomentaryFxConfig };
export type ActiveFx = { cellX: number; cellY: number; fxType: MomentaryFxType; config: MomentaryFxConfig; activatedAtMs: number };
export type AudioCommand =
  | { type: "momentary_fx_start"; id: string; fxType: MomentaryFxType; params: Record<string, unknown>; target: MomentaryFxTarget }
  | { type: "momentary_fx_update"; id: string; params: Record<string, unknown> }
  | { type: "momentary_fx_stop"; id: string }
  | { type: "sample_preview"; instrumentSlot: number; sampleSlot: number; path: string; velocity: number };

export type PartSenseConfig = {
  scanMode: ScanMode;
  scanAxis: ScanAxis;
  scanUnit: NoteUnit;
  scanDirection: Direction;
  scanSections: SectionCount;
  eventEnabled: boolean;
  triggerProbabilityMode: TriggerProbabilityMode;
  triggerProbabilityLowPct: number;
  triggerProbabilityHighPct: number;
  triggerProbabilityMap: TriggerProbabilityCellState[];
  pitch: PitchSettings;
  x: AxisModConfig;
  y: AxisModConfig;
  mapping: {
    activate: { action: TriggerAction; slot: number };
    stable: { action: TriggerAction; slot: number };
    deactivate: { action: TriggerAction; slot: number };
    scanned: { action: TriggerAction; slot: number };
    scanned_empty: { action: TriggerAction; slot: number };
  };
};

export type PartConfig = {
  l1: {
    stepRate: NoteUnit;
    behaviorId: string;
    behaviorConfig: Record<string, unknown>;
    saveGridState: boolean;
    savedState?: unknown;
    triggerGates?: boolean[];
  };
  l2: PartSenseConfig;
  paramMods?: ParamModAxisSlots;
  xy?: { x: AuxTurnBinding | null; y: AuxTurnBinding | null; xInvert?: boolean; yInvert?: boolean };
  autoName: boolean;
  name: string;
};

export type RuntimeConfig = {
  masterVolume: number; displayBrightness: number; gridBrightness: number; buttonBrightness: number; screenSleepSeconds: number;
  midi: { enabled: boolean; outId: string | null; clockOutEnabled: boolean; inId: string | null; clockInEnabled: boolean; syncMode: "internal" | "external"; respondToStartStop: boolean };
  sound: { noteLengthMs: number; velocityScalePct: number; velocityCurve: "linear" | "soft" | "hard"; voiceStealingMode: VoiceStealingMode };
  scanMode: ScanMode; scanAxis: ScanAxis; scanUnit: NoteUnit; scanDirection: Direction; scanSections: SectionCount; algorithmStepUnit: NoteUnit;
  activeBehavior: string; autoSaveDefault: boolean; behaviorConfig: Record<string, unknown>; eventEnabled: boolean; inputEventsWhilePaused: boolean;
  pitch: PitchSettings; x: AxisModConfig; y: AxisModConfig;
  activePartIndex: number; parts: PartConfig[]; numericDisplayMode: NumericDisplayMode; ghostCells: boolean;
  panPositions: number;
  instruments: InstrumentSlotConfig[];
  mixer?: MixerConfig;
  danceMode: DanceMode;
  touchFx?: { selected: MomentaryFxConfig; assignments: FxCellConfig[] };
  auxBindings?: Record<string, AuxBinding | null>;
  xyTouch: { x: number; y: number; active: boolean };
  xyRelease: "sample-hold" | "reset-center";
};

export type ActionSpec =
  | { type: "refresh_presets" } | { type: "preset_save_current" } | { type: "preset_save" }
  | { type: "preset_load"; name: string } | { type: "preset_delete"; name: string }
  | { type: "preset_rename_pick"; name: string } | { type: "preset_rename_apply" }
  | { type: "default_save" } | { type: "default_load" } | { type: "factory_load" }
  | { type: "synth_preset_load"; slot: number; presetId: string; presetLabel: string }
  | { type: "sample_browse_open"; instrumentSlot: number; sampleSlot: number; dir?: string }
  | { type: "sample_browse_up" }
  | { type: "sample_browse_enter"; path: string }
  | { type: "sample_pick"; path: string }
  | { type: "sample_assign_enter"; instrumentSlot: number; sampleSlot: number }
  | { type: "sample_assign_exit" }
  | { type: "trigger_probability_assign_enter"; partIndex: number }
  | { type: "trigger_probability_assign_exit" }
  | { type: "fx_assign_enter"; config: MomentaryFxConfig }
  | { type: "fx_assign_exit" }
  | { type: "midi_select_output"; id: string | null } | { type: "midi_select_input"; id: string | null }
  | { type: "midi_panic" } | { type: "behavior_action"; behaviorId: string; actionType: string }
  | { type: "instrument_clone"; slot: number } | { type: "instrument_reset"; slot: number }
  | { type: "xy_set_target"; axis: "x" | "y"; binding: AuxTurnBinding | null }
  | { type: "noop" }
  | { type: "menu_back" };

export type MenuState = { stack: number[]; cursor: number; editing: boolean };
export type SavedSystemConfig = { danceMode?: DanceMode; touchMode?: DanceMode; triggerGateTarget?: "active" | "all" | string };
export type ConfigPayload = { activeBehavior: string; runtimeConfig: RuntimeConfig; mappingConfig: MappingConfig; system?: SavedSystemConfig };
export type ConfirmKind = "overwrite_preset" | "delete_preset" | "rename_preset" | "load_preset" | "load_default" | "load_factory" | "save_default" | "load_synth_preset" | "text_dirty_exit" | "midi_panic" | "aux_unbind" | "help_info";
type TextConfirmMode = "save" | "discard";
export type PendingAction =
  | { kind: "preset_save"; name: string } | { kind: "preset_delete"; name: string } | { kind: "preset_load"; name: string }
  | { kind: "preset_rename"; from: string; to: string } | { kind: "default_save" } | { kind: "default_load" } | { kind: "factory_load" } | { kind: "midi_panic" }
  | { kind: "synth_preset_load"; slot: number; presetId: string; presetLabel: string }
  | { kind: "aux_unbind"; encoderId: string } | { kind: "help_info"; title: string; lines: string[] }
  | { kind: "text_dirty_exit"; key: string; original: string; saveAction?: ActionSpec; backAfter: boolean; mode: TextConfirmMode };
export type ConfirmState = { kind: ConfirmKind; action: PendingAction; cursor: number; options: string[]; scroll: number };
export type TextEditSession = { key: string; original: string; saveAction?: ActionSpec };
export type ToastState = { message: string; startedAtMs: number; untilMs: number };
export type MidiPortInfo = { id: string; name: string };
export type AuxTurnBinding = { key: string; label?: string; kind: "number" | "enum" | "bool"; min?: number; max?: number; step?: number; options?: string[] };
export type AuxPressBinding =
  | { kind: "behavior_action"; actionType: string; routeKey?: string; label?: string }
  | { kind: "menu_action"; action: ActionSpec; label?: string };
export type AuxBinding = { turn: AuxTurnBinding | null; press: AuxPressBinding | null };
export type SystemState = {
  shiftHeld: boolean; fnHeld: boolean; physicalShiftHeld: boolean; physicalFnHeld: boolean; combinedModifierHeld: boolean; presetNames: string[]; selectedPreset: string | null; currentPresetName: string | null; draftName: string; nameCursor: number;
  pendingRename: { from: string; to: string } | null; confirm: ConfirmState | null; toast: ToastState | null; eventBlipUntilMs: number; stopLatched: boolean;
  transportFlash: "none" | "beat" | "measure"; transportFlashUntilMs: number; autoSaveFlash: "none" | "flash"; autoSaveFlashUntilMs: number; textEdit: TextEditSession | null;
  midiOutputs: MidiPortInfo[]; midiInputs: MidiPortInfo[]; midiStatus: string | null; externalPpqnPulse: number; pendingResync: boolean; pausedByUser: boolean;
  oledMode: "normal" | "splash" | "off"; oledSplashText: string; oledSplashUntilMs: number; lastInteractionMs: number; auxBindings: Record<string, AuxBinding | null>;
  shiftHeldSinceMs: number | null;
  auxOverlayScroll: number;
  auxAutoMapEnabled: boolean;
  heldNotes: string[];
  pendingCloneSource: number | null;
  sampleAssign: { instrumentSlot: number; sampleSlot: number } | null;
  triggerProbabilityAssign: { partIndex: number } | null;
  fxAssignMode: { config: MomentaryFxConfig } | null;
  activeFx: ActiveFx[];
  sampleBrowser: {
    instrumentSlot: number;
    sampleSlot: number;
    dir: string;
    entries: Array<{ name: string; path: string; isDir: boolean }>;
  } | null;
  danceMode: DanceMode;
  triggerGateTarget: "active" | "all" | string;
  triggerMuted: boolean;
  triggerGateRestoreModes: Array<TriggerProbabilityMode | null>;
};

export type PlatformEffectBase =
  | { type: "store_list_presets" } | { type: "store_load_preset"; name: string } | { type: "store_save_preset"; name: string; payload: ConfigPayload }
  | { type: "store_delete_preset"; name: string } | { type: "store_load_default" } | { type: "store_save_default"; payload: ConfigPayload; mode?: "immediate" | "deferred" };
export type MidiEffect =
  | { type: "midi_list_outputs_request" } | { type: "midi_list_inputs_request" }
  | { type: "midi_select_output"; id: string | null } | { type: "midi_select_input"; id: string | null } | { type: "midi_panic" };
export type SampleEffect =
  | { type: "sample_list_request"; instrumentSlot: number; sampleSlot: number; dir: string };
export type AudioCommandEffect = { type: "audio_command"; command: AudioCommand };
export type PlatformEffect = PlatformEffectBase | MidiEffect | SampleEffect | AudioCommandEffect;
export type StoreResultBase =
  | { type: "list_presets_result"; names: string[] } | { type: "load_preset_result"; name: string; payload: ConfigPayload | null }
  | { type: "save_preset_result"; name: string; outcome: "created" | "overwritten" } | { type: "delete_preset_result"; name: string; ok: boolean }
  | { type: "load_default_result"; payload: ConfigPayload | null } | { type: "save_default_result"; ok: boolean; isAuto?: boolean } | { type: "store_error"; message: string };
export type MidiResult =
  | { type: "midi_list_outputs_result"; outputs: MidiPortInfo[] }
  | { type: "midi_list_inputs_result"; inputs: MidiPortInfo[] }
  | { type: "midi_status"; ok: boolean; message?: string; selectedOutId?: string | null; selectedInId?: string | null };
export type SampleResult =
  | { type: "sample_list_result"; instrumentSlot: number; sampleSlot: number; dir: string; entries: Array<{ name: string; path: string; isDir: boolean }> }
  | { type: "sample_list_error"; instrumentSlot: number; sampleSlot: number; dir: string; message: string }
  | { type: "sample_preview_error"; message: string };
export type StoreResult = StoreResultBase | MidiResult | SampleResult;

export type PlatformState<TState> = {
  transport: TransportFrame; behaviorState: TState; activeBehavior: string; mappingConfig: MappingConfig; runtimeConfig: RuntimeConfig; menu: MenuState; system: SystemState;
  scanIndex: number; scanPulseAccumulator: number; algorithmPulseAccumulator: number; ppqnPulseRemainder: number;
  partStates: unknown[]; partScanIndex: number[]; partScanPulseAccumulator: number[]; partAlgorithmPulseAccumulator: number[];
};

export type MenuNode =
  | { kind: "group"; label: string; children: MenuNode[] | ((state: PlatformState<any>) => MenuNode[]); visible?: (c: RuntimeConfig) => boolean; flat?: boolean | ((config: RuntimeConfig) => boolean); detail?: (state: PlatformState<any>) => string | null }
  | { kind: "enum"; label: string; key: string; options: string[]; visible?: (c: RuntimeConfig) => boolean }
  | { kind: "number"; label: string; key: string; min: number; max: number; step: number; displayStyle?: "number" | "bar" | "marker"; visible?: (c: RuntimeConfig) => boolean }
  | { kind: "bool"; label: string; key: string; visible?: (c: RuntimeConfig) => boolean }
  | { kind: "action"; label: string; action: ActionSpec }
  | { kind: "text"; label: string; key: string; maxLen: number; onExitSaveAction?: ActionSpec }
  | { kind: "spacer" };

export const OLED_WIDTH = 128;
export const OLED_HEIGHT = 128;
export const OLED_TEXT_COLUMNS = 20;
export const OLED_TEXT_LINES = 8;
