const API = {
  networks: '/networks',
  country: 'http://192.168.42.1:8080/country',
  connect: '/connect',
  stage: 'http://192.168.42.1:8080/stage',
};

const OPEN_SECURITY = ['open', 'none', 'nopass', 'unsecured'];
const COUNTRY_INVALID = 'Enter a two-letter Wi-Fi country code before scanning.';
const COUNTRY_APPLY_FAILED = 'Could not apply the Wi-Fi country. Check the code and try again.';
const SCAN_FAILED = 'Could not scan nearby Wi-Fi networks. Enter the SSID manually if needed.';
const APPLYING_PENDING = 'Setup is being applied. Watch the OLED for the final result.';
const APPLYING_REJECTED = (status) => `The device rejected the Wi-Fi request (HTTP ${status}).`;
const DISCONNECT_GUIDANCE = 'The portal connection may have dropped while setup applies. This can be expected. Watch the OLED for the final result. After success, find the address in System > Info.';
const PASSWORD_MIN_LENGTH = 8;
const PASSWORD_MAX_LENGTH = 128;
const PASSWORD_WHITESPACE_RANGES = [
  [0x0009, 0x000d],
  [0x0020, 0x0020],
  [0x0085, 0x0085],
  [0x00a0, 0x00a0],
  [0x1680, 0x1680],
  [0x2000, 0x200a],
  [0x2028, 0x2029],
  [0x202f, 0x202f],
  [0x205f, 0x205f],
  [0x3000, 0x3000],
];
const PASSWORD_CONTROL_RANGES = [
  [0x0000, 0x001f],
  [0x007f, 0x009f],
  [0x00ad, 0x00ad],
  [0x061c, 0x061c],
  [0x180e, 0x180e],
  [0x200b, 0x200f],
  [0x202a, 0x202e],
  [0x2060, 0x206f],
  [0xfeff, 0xfeff],
];

const passwordCodePointInRanges = (codePoint, ranges) => ranges.some(([start, end]) => codePoint >= start && codePoint <= end);

const passwordWhitespaceOnly = (value) => {
  const codePoints = Array.from(value);
  return codePoints.length > 0 && codePoints.every((character) => passwordCodePointInRanges(character.codePointAt(0), PASSWORD_WHITESPACE_RANGES));
};

const passwordHasControl = (value) => Array.from(value).some((character) => passwordCodePointInRanges(character.codePointAt(0), PASSWORD_CONTROL_RANGES));

const state = {
  networks: [],
  networkError: '',
  selectedSsid: '',
  manualSsid: '',
  wifiPassphrase: '',
  wifiCountry: 'US',
  openNetwork: false,
  accountPassword: '',
  accountPasswordConfirm: '',
  sshMode: 'key',
  sshPublicKey: '',
};

const els = {
  errors: document.getElementById('errors'),
  form: document.getElementById('setupForm'),
  applyButton: document.getElementById('applyButton'),
  refreshNetworks: document.getElementById('refreshNetworks'),
  networkState: document.getElementById('networkState'),
  networkList: document.getElementById('networkList'),
  manualSsid: document.getElementById('manualSsid'),
  wifiPassphrase: document.getElementById('wifiPassphrase'),
  wifiCountry: document.getElementById('wifiCountry'),
  openNetwork: document.getElementById('openNetwork'),
  accountPassword: document.getElementById('accountPassword'),
  accountPasswordConfirm: document.getElementById('accountPasswordConfirm'),
  sshKeyFields: document.getElementById('sshKeyFields'),
  sshPublicKey: document.getElementById('sshPublicKey'),
  passwordAccessHint: document.getElementById('passwordAccessHint'),
  noneAccessHint: document.getElementById('noneAccessHint'),
  hostname: document.getElementById('hostname'),
  applyingPanel: document.getElementById('applyingPanel'),
  applyingStatus: document.getElementById('applyingStatus'),
};

const escapeHtml = (value) =>
  String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');

const networkSecurity = (network) => String(network.security ?? network.sec ?? network.auth ?? network.encryption ?? '').toLowerCase();

const normalizeNetwork = (network) => ({
  ssid: network.ssid ?? network.SSID ?? network.name ?? '',
  security: networkSecurity(network),
  signal: Number(network.signal ?? network.rssi ?? network.strength ?? Number.NEGATIVE_INFINITY),
});

const normalizeNetworks = (payload) => {
  const list = Array.isArray(payload) ? payload : Array.isArray(payload?.networks) ? payload.networks : [];
  return list.map(normalizeNetwork).filter((network) => network.ssid);
};

const effectiveSsid = () => state.manualSsid || state.selectedSsid;

const selectedNetwork = () => (state.manualSsid ? undefined : state.networks.find((network) => network.ssid === effectiveSsid()));

const requiresWifiPassword = () => {
  if (state.openNetwork) {
    return false;
  }
  return !OPEN_SECURITY.includes(selectedNetwork()?.security ?? '');
};

const signalLabel = (signal) => {
  if (!Number.isFinite(signal)) {
    return 'Weak signal';
  }
  if (signal < 0) {
    return `${signal >= -55 ? 'Strong' : signal >= -70 ? 'Fair' : 'Weak'} signal`;
  }
  return `${signal >= 67 ? 'Strong' : signal >= 34 ? 'Fair' : 'Weak'} signal`;
};

const networkMeta = (network) => {
  const security = OPEN_SECURITY.includes(network.security) ? 'Open' : 'Locked';
  return `${security} · ${signalLabel(network.signal)}`;
};

const renderNetworks = () => {
  if (state.networkPhase === 'country') {
    els.networkState.textContent = 'Applying Wi-Fi country…';
  } else if (state.networkPhase === 'scan') {
    els.networkState.textContent = 'Scanning for nearby Wi-Fi networks…';
  } else if (state.networkError) {
    els.networkState.textContent = state.networkError;
  } else if (state.networks.length) {
    els.networkState.textContent = `${state.networks.length} network${state.networks.length === 1 ? '' : 's'} found.`;
  } else {
    els.networkState.textContent = 'No networks found. You can still enter the SSID manually.';
  }
  const loading = ['country', 'scan'].includes(state.networkPhase);
  els.networkState.dataset.loading = String(loading);
  els.networkList.setAttribute('aria-busy', String(loading));
  els.refreshNetworks.disabled = loading;

  const items = state.networks
    .map(
      (network) => `
        <label class="network-item${!state.manualSsid && state.selectedSsid === network.ssid ? ' is-selected' : ''}" title="${escapeHtml(network.ssid)}">
          <input class="network-radio" type="radio" name="ssidChoice" value="${escapeHtml(network.ssid)}" aria-label="Select ${escapeHtml(network.ssid)}" ${!state.manualSsid && state.selectedSsid === network.ssid ? 'checked' : ''} />
          <span class="network-radio-mark" aria-hidden="true"></span>
          <span class="network-copy">
            <strong class="network-ssid" title="${escapeHtml(network.ssid)}">${escapeHtml(network.ssid)}</strong>
            <span class="network-meta">${escapeHtml(networkMeta(network))}</span>
          </span>
        </label>`,
    )
    .join('');

  els.networkList.innerHTML = items || '<div class="muted">No networks were returned. Enter the SSID manually below.</div>';
  els.wifiPassphrase.disabled = !requiresWifiPassword();
  els.openNetwork.checked = state.openNetwork;
};

const renderSshChoices = () => {
  document.querySelectorAll('.ssh-choice').forEach((choice) => {
    const input = choice.querySelector('input[name="sshMode"]');
    const selected = input?.value === state.sshMode;
    choice.classList.toggle('is-selected', selected);
    if (input) {
      input.checked = selected;
    }
  });
};

const syncStateFromInputs = () => {
  state.manualSsid = els.manualSsid.value.trim();
  state.wifiPassphrase = els.wifiPassphrase.value;
  state.wifiCountry = els.wifiCountry.value.trim().toUpperCase();
  state.openNetwork = els.openNetwork.checked;
  state.accountPassword = els.accountPassword.value;
  state.accountPasswordConfirm = els.accountPasswordConfirm.value;
  state.sshPublicKey = els.sshPublicKey.value.trim();
  state.sshMode = document.querySelector('input[name="sshMode"]:checked')?.value ?? 'none';
};

const clearScannedSelection = () => {
  state.selectedSsid = '';
  els.networkList.querySelectorAll('input[name="ssidChoice"]').forEach((input) => {
    input.checked = false;
  });
};

const focusSelectedNetwork = () => {
  const radio = Array.from(els.networkList.querySelectorAll('input[name="ssidChoice"]')).find((input) => input.value === state.selectedSsid);
  if (!radio) {
    return;
  }
  try {
    radio.focus({ preventScroll: true });
  } catch {
    radio.focus();
  }
};

const render = () => {
  renderNetworks();
  renderSshChoices();
  els.sshKeyFields.hidden = state.sshMode !== 'key';
  els.sshPublicKey.disabled = state.sshMode !== 'key';
  els.passwordAccessHint.hidden = state.sshMode !== 'password';
  els.noneAccessHint.hidden = state.sshMode !== 'none';
};

const validateSsh = () => {
  if (state.sshMode === 'key' && !state.sshPublicKey) return { field: els.sshPublicKey, message: 'Paste an SSH public key or choose another SSH mode.' };
  return undefined;
};

const validateAccountPassword = () => {
  const length = Array.from(state.accountPassword).length;
  if (length < PASSWORD_MIN_LENGTH) return { field: els.accountPassword, message: 'Choose a local device password with at least 8 characters.' };
  if (length > PASSWORD_MAX_LENGTH) return { field: els.accountPassword, message: 'Choose a local device password with no more than 128 characters.' };
  if (passwordWhitespaceOnly(state.accountPassword)) return { field: els.accountPassword, message: 'Choose a local device password that is not only whitespace.' };
  if (passwordHasControl(state.accountPassword)) return { field: els.accountPassword, message: 'Local device password cannot contain control characters.' };
  if (state.accountPassword !== state.accountPasswordConfirm) return { field: els.accountPasswordConfirm, message: 'Local device password confirmation does not match.' };
  return undefined;
};

const validationError = () => {
  syncStateFromInputs();
  if (!/^[A-Z]{2}$/.test(state.wifiCountry)) {
    return { field: els.wifiCountry, message: 'Enter a two-letter Wi-Fi country code.' };
  }
  if (!effectiveSsid()) {
    return { field: els.manualSsid, message: 'Choose a Wi-Fi network or enter the SSID manually.' };
  }
  if (requiresWifiPassword() && !state.wifiPassphrase) {
    return { field: els.wifiPassphrase, message: 'This network needs a Wi-Fi password.' };
  }
  const accountPasswordError = validateAccountPassword();
  if (accountPasswordError) return accountPasswordError;
  return validateSsh();
};

const stagePayload = () => ({
  sshMode: state.sshMode,
  sshPublicKey: state.sshMode === 'key' ? state.sshPublicKey : '',
  accountPassword: state.accountPassword,
  accountPasswordConfirm: state.accountPasswordConfirm,
  hostname: els.hostname.value.trim(),
  wifiCountry: state.wifiCountry,
});

const connectPayload = () =>
  new URLSearchParams({
    ssid: effectiveSsid(),
    identity: '',
    passphrase: state.openNetwork ? '' : state.wifiPassphrase,
  });

const stageFailureMessage = async (response) => {
  const statusMessage = `Setup stage failed (HTTP ${response.status}).`;
  if (response.status !== 400) {
    return statusMessage;
  }
  try {
    const payload = await response.json();
    if (payload?.error === 'invalid_input') {
      return `${statusMessage} Could not apply setup settings. Check the form and try again.`;
    }
  } catch {}
  return statusMessage;
};

const loadNetworks = async () => {
  state.networkError = '';
  state.networkPhase = 'country';
  renderNetworks();
  try {
    syncStateFromInputs();
    if (!/^[A-Z]{2}$/.test(state.wifiCountry)) {
      state.networkError = COUNTRY_INVALID;
      return;
    }
    const countryResponse = await fetch(API.country, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ wifiCountry: state.wifiCountry }),
    });
    if (!countryResponse.ok) {
      state.networkError = COUNTRY_APPLY_FAILED;
      return;
    }
    state.networkPhase = 'scan';
    renderNetworks();
    const response = await fetch(API.networks, { cache: 'no-store' });
    if (!response.ok) {
      throw new Error(`Network scan failed (${response.status})`);
    }
    state.networks = normalizeNetworks(await response.json());
  } catch (error) {
    state.networkError = state.networkPhase === 'scan' ? SCAN_FAILED : COUNTRY_APPLY_FAILED;
  } finally {
    state.networkPhase = 'idle';
    render();
  }
};

const submit = async (event) => {
  event.preventDefault();
  els.errors.textContent = '';
  const invalid = validationError();
  if (invalid) {
    els.errors.textContent = invalid.message;
    invalid.field.focus();
    return;
  }

  els.applyButton.disabled = true;
  try {
    let stageResponse;
    try {
      stageResponse = await fetch(API.stage, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(stagePayload()),
      });
    } catch (error) {
      els.errors.textContent = 'Could not apply setup settings. Check the form and try again.';
      return;
    }
    if (!stageResponse.ok) {
      els.errors.textContent = await stageFailureMessage(stageResponse);
      return;
    }

    els.applyingStatus.textContent = APPLYING_PENDING;
    els.applyingStatus.hidden = false;
    els.form.hidden = true;
    els.applyingPanel.hidden = false;
    try {
      const connectResponse = await fetch(API.connect, {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: connectPayload(),
      });
      if (!connectResponse.ok) {
        els.applyingStatus.textContent = APPLYING_REJECTED(connectResponse.status);
      }
    } catch (error) {
      els.applyingStatus.textContent = DISCONNECT_GUIDANCE;
    }
  } finally {
    els.applyButton.disabled = false;
  }
};

const bindEvents = () => {
  els.refreshNetworks.addEventListener('click', () => loadNetworks());
  els.form.addEventListener('change', (event) => {
    const target = event.target;
    els.errors.textContent = '';
    if (target.name === 'sshMode') {
      state.sshMode = target.value;
      render();
      return;
    }
    if (target.name === 'ssidChoice') {
      els.manualSsid.value = '';
      state.manualSsid = '';
      state.selectedSsid = target.value;
      state.openNetwork = OPEN_SECURITY.includes(selectedNetwork()?.security ?? '');
      render();
      focusSelectedNetwork();
      return;
    } else {
      syncStateFromInputs();
    }
    render();
  });
  els.form.addEventListener('input', (event) => {
    if (event.target.matches('input[name="ssidChoice"]')) {
      return;
    }
    syncStateFromInputs();
    els.errors.textContent = '';
    if (event.target.id === 'manualSsid' && state.manualSsid) {
      clearScannedSelection();
      state.openNetwork = false;
    }
    render();
  });
  els.form.addEventListener('submit', submit);
};

const init = async () => {
  bindEvents();
  render();
  await loadNetworks();
};

init();
