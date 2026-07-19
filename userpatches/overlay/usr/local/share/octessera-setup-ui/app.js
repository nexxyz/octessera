const API = {
  networks: '/networks',
  connect: '/connect',
  stage: 'http://192.168.42.1:8080/stage',
};

const STEPS = [0, 1, 2, 3, 4, 5];

const state = {
  step: 0,
  networks: [],
  loadingNetworks: false,
  networkError: '',
  selectedSsid: '',
  selectedSecurity: '',
  manualSsid: '',
  wifiPassphrase: '',
  wifiCountry: 'US',
  openNetwork: false,
  sshMode: 'key',
  sshPublicKey: '',
  sshPassword: '',
  sshPasswordConfirm: '',
  hostname: '',
  submitting: false,
  status: '',
  error: '',
};

const els = {
  progressTrack: document.getElementById('progressTrack'),
  stepDots: document.getElementById('stepDots'),
  status: document.getElementById('status'),
  errors: document.getElementById('errors'),
  form: document.getElementById('setupForm'),
  startButton: document.getElementById('startButton'),
  refreshNetworks: document.getElementById('refreshNetworks'),
  networkState: document.getElementById('networkState'),
  networkList: document.getElementById('networkList'),
  manualSsid: document.getElementById('manualSsid'),
  wifiPassphrase: document.getElementById('wifiPassphrase'),
  wifiCountry: document.getElementById('wifiCountry'),
  openNetwork: document.getElementById('openNetwork'),
  sshKeyFields: document.getElementById('sshKeyFields'),
  sshPasswordFields: document.getElementById('sshPasswordFields'),
  sshPublicKey: document.getElementById('sshPublicKey'),
  sshPassword: document.getElementById('sshPassword'),
  sshPasswordConfirm: document.getElementById('sshPasswordConfirm'),
  hostname: document.getElementById('hostname'),
  review: document.getElementById('review'),
};

const escapeHtml = (value) =>
  String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');

const normalizeNetworks = (payload) => {
  const list = Array.isArray(payload) ? payload : Array.isArray(payload?.networks) ? payload.networks : [];
  return list
    .map((network) => {
      const ssid = network.ssid ?? network.SSID ?? network.name ?? '';
      const security = String(network.security ?? network.sec ?? network.auth ?? network.encryption ?? '').toLowerCase();
      const signal = Number(network.signal ?? network.rssi ?? network.strength ?? Number.NEGATIVE_INFINITY);
      return { ssid, security, signal };
    })
    .filter((network) => network.ssid)
    .sort((a, b) => b.signal - a.signal || a.ssid.localeCompare(b.ssid));
};

const setStatus = (message) => {
  state.status = message;
  renderMessages();
};

const setError = (message) => {
  state.error = message;
  renderMessages();
};

const renderMessages = () => {
  els.status.textContent = state.status;
  els.errors.textContent = state.error;
};

const setStep = (nextStep) => {
  state.step = Math.max(0, Math.min(STEPS.length - 1, nextStep));
  render();
};

const syncStateFromInputs = () => {
  state.manualSsid = els.manualSsid.value.trim();
  state.wifiPassphrase = els.wifiPassphrase.value;
  state.wifiCountry = els.wifiCountry.value.trim().toUpperCase();
  state.openNetwork = els.openNetwork.checked;
  state.sshPublicKey = els.sshPublicKey.value.trim();
  state.sshPassword = els.sshPassword.value;
  state.sshPasswordConfirm = els.sshPasswordConfirm.value;
  state.hostname = els.hostname.value.trim();
  state.sshMode = document.querySelector('input[name="sshMode"]:checked')?.value ?? 'none';
};

const selectedNetwork = () => state.networks.find((network) => network.ssid === state.selectedSsid);

const chosenSsid = () => state.manualSsid || state.selectedSsid;

const requiresWifiPassword = () => {
  if (state.openNetwork) {
    return false;
  }
  const network = selectedNetwork();
  if (!network) {
    return true;
  }
  if (!network.security) {
    return true;
  }
  return !['open', 'none', 'nopass', 'unsecured'].includes(network.security);
};

const networkLabel = (network) => {
  const security = network.security && !['open', 'none', 'nopass', 'unsecured'].includes(network.security) ? 'secured' : 'open';
  return `${network.ssid} · ${security}`;
};

const renderNetworks = () => {
  if (state.loadingNetworks) {
    els.networkState.textContent = 'Scanning for nearby Wi-Fi networks…';
  } else if (state.networkError) {
    els.networkState.textContent = state.networkError;
  } else if (state.networks.length) {
    els.networkState.textContent = `${state.networks.length} network${state.networks.length === 1 ? '' : 's'} found.`;
  } else {
    els.networkState.textContent = 'No scan results yet. You can still enter the SSID manually.';
  }

  const items = state.networks
    .map(
      (network) => `
        <label class="network-item">
          <input type="radio" name="ssidChoice" value="${escapeHtml(network.ssid)}" ${state.selectedSsid === network.ssid ? 'checked' : ''} />
          <span>
            <strong>${escapeHtml(network.ssid)}</strong>
            <span class="network-meta">${escapeHtml(networkLabel(network))}</span>
          </span>
        </label>`,
    )
    .join('');

  els.networkList.innerHTML = items || '<div class="muted">No networks were returned.</div>';
  els.networkList.querySelectorAll('input[name="ssidChoice"]').forEach((input) => {
    input.addEventListener('change', (event) => {
      state.selectedSsid = event.target.value;
      const network = selectedNetwork();
      state.selectedSecurity = network?.security ?? '';
      renderNetworks();
    });
  });

  const network = selectedNetwork();
  const shouldRequirePassword = requiresWifiPassword();
  els.wifiPassphrase.disabled = !shouldRequirePassword;
  els.openNetwork.checked = !shouldRequirePassword;
  if (network && ['open', 'none', 'nopass', 'unsecured'].includes(network.security)) {
    els.openNetwork.checked = true;
  }
};

const renderProgress = () => {
  const percent = ((state.step + 1) / STEPS.length) * 100;
  els.progressTrack.style.setProperty('--progress', `${percent}%`);
  els.stepDots.innerHTML = STEPS.slice(0, -1)
    .map((step) => `<li class="${step === state.step ? 'active' : ''}"></li>`)
    .join('');
};

const renderReview = () => {
  const values = {
    Network: chosenSsid() || 'Not selected',
    'Wi-Fi password': requiresWifiPassword() ? 'Set' : 'Not needed',
    Country: state.wifiCountry || 'Not set',
    'SSH mode': state.sshMode,
    'SSH public key': state.sshMode === 'key' ? (state.sshPublicKey || 'Missing') : 'Not used',
    'SSH password': state.sshMode === 'password' ? 'Set' : 'Not used',
    Hostname: state.hostname || 'Default',
  };

  els.review.innerHTML = Object.entries(values)
    .map(([label, value]) => `<div class="review-row"><strong>${escapeHtml(label)}</strong><span>${escapeHtml(value)}</span></div>`)
    .join('');
};

const renderStepFields = () => {
  document.querySelectorAll('[data-step]').forEach((section) => {
    section.hidden = Number(section.dataset.step) !== state.step;
  });

  renderProgress();
  renderNetworks();
  renderReview();

  const currentMode = state.sshMode;
  els.sshKeyFields.hidden = currentMode !== 'key';
  els.sshPasswordFields.hidden = currentMode !== 'password';
  els.wifiCountry.value = state.wifiCountry;
  els.manualSsid.value = state.manualSsid;
  els.wifiPassphrase.value = state.wifiPassphrase;
  els.openNetwork.checked = state.openNetwork;
  els.sshPublicKey.value = state.sshPublicKey;
  els.sshPassword.value = state.sshPassword;
  els.sshPasswordConfirm.value = state.sshPasswordConfirm;
  els.hostname.value = state.hostname;
};

const render = () => {
  renderMessages();
  renderStepFields();
  const progress = state.submitting ? 'Submitting configuration…' : state.step === 1 ? 'Pick Wi-Fi and country.' : state.step === 2 ? 'Choose SSH access.' : state.step === 3 ? 'Give the device a name, if you want.' : state.step === 4 ? 'Review the plan.' : '';
  if (progress) {
    els.status.textContent = progress;
  }
};

const validateWifiStep = () => {
  syncStateFromInputs();
  state.selectedSsid = state.selectedSsid || state.manualSsid;
  state.selectedSecurity = selectedNetwork()?.security ?? '';
  const ssid = chosenSsid();
  if (!ssid) {
    return 'Choose a Wi-Fi network or enter the SSID manually.';
  }
  if (!state.wifiCountry || state.wifiCountry.length !== 2) {
    return 'Enter a two-letter Wi-Fi country code.';
  }
  if (requiresWifiPassword() && !state.wifiPassphrase) {
    return 'This network needs a Wi-Fi password.';
  }
  return '';
};

const validateSshStep = () => {
  syncStateFromInputs();
  if (state.sshMode === 'key' && !state.sshPublicKey) {
    return 'Paste an SSH public key or switch to password access.';
  }
  if (state.sshMode === 'password') {
    if (state.sshPassword.length < 12) {
      return 'SSH passwords need at least 12 characters.';
    }
    if (state.sshPassword !== state.sshPasswordConfirm) {
      return 'SSH password confirmation does not match.';
    }
  }
  return '';
};

const stagePayload = () => ({
  sshMode: state.sshMode,
  sshPublicKey: state.sshMode === 'key' ? state.sshPublicKey : '',
  sshPassword: state.sshMode === 'password' ? state.sshPassword : '',
  sshPasswordConfirm: state.sshMode === 'password' ? state.sshPasswordConfirm : '',
  hostname: state.hostname,
  wifiCountry: state.wifiCountry,
});

const connectPayload = () => {
  const identity = '';
  return new URLSearchParams({
    ssid: chosenSsid(),
    identity,
    passphrase: state.openNetwork ? '' : state.wifiPassphrase,
  });
};

const loadNetworks = async () => {
  state.loadingNetworks = true;
  state.networkError = '';
  renderNetworks();
  try {
    const response = await fetch(API.networks, { cache: 'no-store' });
    if (!response.ok) {
      throw new Error(`Network scan failed (${response.status})`);
    }
    const payload = await response.json();
    state.networks = normalizeNetworks(payload);
    if (!state.selectedSsid && state.networks[0]) {
      state.selectedSsid = state.networks[0].ssid;
      state.selectedSecurity = state.networks[0].security;
    }
  } catch (error) {
    state.networkError = 'Could not scan the nearby Wi-Fi list. Enter the SSID manually if needed.';
    state.networks = [];
  } finally {
    state.loadingNetworks = false;
    render();
  }
};

const goNext = () => {
  setError('');
  if (state.step === 0) {
    setStep(1);
    return;
  }
  if (state.step === 1) {
    const error = validateWifiStep();
    if (error) {
      setError(error);
      return;
    }
    setStep(2);
    return;
  }
  if (state.step === 2) {
    const error = validateSshStep();
    if (error) {
      setError(error);
      return;
    }
    setStep(3);
    return;
  }
  if (state.step === 3) {
    setStep(4);
  }
};

const goBack = () => setStep(state.step - 1);

const submit = async (event) => {
  event.preventDefault();
  setError('');
  syncStateFromInputs();
  const wifiError = validateWifiStep();
  if (wifiError) {
    setError(wifiError);
    setStep(1);
    return;
  }
  const sshError = validateSshStep();
  if (sshError) {
    setError(sshError);
    setStep(2);
    return;
  }

  state.submitting = true;
  render();
  try {
    setStatus('Saving device settings…');
    const stageResponse = await fetch(API.stage, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(stagePayload()),
    });
    if (!stageResponse.ok) {
      throw new Error(`Setup stage failed (${stageResponse.status})`);
    }

    setStatus('Joining Wi-Fi…');
    const connectResponse = await fetch(API.connect, {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: connectPayload(),
    });
    if (!connectResponse.ok) {
      throw new Error(`Wi-Fi connection failed (${connectResponse.status})`);
    }

    setStep(5);
    setStatus('Setup request sent. The hotspot should disappear once the device joins Wi-Fi.');
  } catch (error) {
    setError(error instanceof Error ? error.message : 'Setup failed.');
    setStatus('');
  } finally {
    state.submitting = false;
    render();
  }
};

const bindEvents = () => {
  els.startButton.addEventListener('click', () => setStep(1));
  els.refreshNetworks.addEventListener('click', () => loadNetworks());
  els.form.addEventListener('click', (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    if (target.matches('[data-next]')) {
      goNext();
    }
    if (target.matches('[data-back]')) {
      goBack();
    }
  });

  els.form.addEventListener('change', (event) => {
    const target = event.target;
    if (!(target instanceof HTMLElement)) {
      return;
    }
    syncStateFromInputs();
    if (target.name === 'sshMode') {
      render();
    }
    if (target.name === 'ssidChoice') {
      state.selectedSsid = target.value;
      state.manualSsid = '';
      els.manualSsid.value = '';
      render();
    }
    if (target.id === 'openNetwork') {
      render();
    }
  });

  ['input', 'blur'].forEach((eventName) => {
    els.form.addEventListener(eventName, () => {
      syncStateFromInputs();
      if (state.step === 1 || state.step === 2 || state.step === 3 || state.step === 4) {
        renderReview();
      }
      renderNetworks();
    });
  });

  els.form.addEventListener('submit', submit);
};

const init = async () => {
  bindEvents();
  render();
  await loadNetworks();
};

init();
