const state = {
  step: 0,
  networks: [],
  selectedSsid: '',
  manualSsid: '',
  wifiPassphrase: '',
  wifiCountry: 'US',
  openNetwork: false,
  sshMode: 'key',
  sshPublicKey: '',
  sshPassword: '',
  sshPasswordConfirm: '',
  hostname: '',
  busy: false,
  error: '',
  status: '',
};

const byId = (id) => document.getElementById(id);
const form = byId('setupForm');
const escapeHtml = (value) => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;').replaceAll("'", '&#39;');
const selectedNetwork = () => state.networks.find((network) => network.ssid === state.selectedSsid);
const ssid = () => state.manualSsid || state.selectedSsid;
const openSecurity = () => state.openNetwork || ['open', 'none', 'nopass', 'unsecured'].includes(selectedNetwork()?.security ?? '');

const sync = () => {
  state.manualSsid = byId('manualSsid').value.trim();
  state.wifiPassphrase = byId('wifiPassphrase').value;
  state.wifiCountry = byId('wifiCountry').value.trim().toUpperCase();
  state.openNetwork = byId('openNetwork').checked;
  state.sshPublicKey = byId('sshPublicKey').value.trim();
  state.sshPassword = byId('sshPassword').value;
  state.sshPasswordConfirm = byId('sshPasswordConfirm').value;
  state.hostname = byId('hostname').value.trim();
  state.sshMode = document.querySelector('input[name="sshMode"]:checked')?.value ?? 'none';
};

const message = () => {
  byId('status').textContent = state.status;
  byId('errors').textContent = state.error;
};

const render = () => {
  document.querySelectorAll('[data-step]').forEach((section) => { section.hidden = Number(section.dataset.step) !== state.step; });
  const progress = ((state.step + 1) / 6) * 100;
  byId('progressTrack').style.setProperty('--progress', `${progress}%`);
  byId('stepDots').innerHTML = [0, 1, 2, 3, 4].map((step) => `<li class="${step === state.step ? 'active' : ''}"></li>`).join('');
  byId('sshKeyFields').hidden = state.sshMode !== 'key';
  byId('sshPasswordFields').hidden = state.sshMode !== 'password';
  byId('manualSsid').value = state.manualSsid;
  byId('wifiPassphrase').value = state.wifiPassphrase;
  byId('wifiCountry').value = state.wifiCountry;
  byId('openNetwork').checked = state.openNetwork;
  byId('sshPublicKey').value = state.sshPublicKey;
  byId('sshPassword').value = state.sshPassword;
  byId('sshPasswordConfirm').value = state.sshPasswordConfirm;
  byId('hostname').value = state.hostname;
  byId('wifiPassphrase').disabled = openSecurity();
  byId('networkList').innerHTML = state.networks.length ? state.networks.map((network) => `<label class="network-item"><input type="radio" name="ssidChoice" value="${escapeHtml(network.ssid)}" ${network.ssid === state.selectedSsid ? 'checked' : ''} /><span><strong>${escapeHtml(network.ssid)}</strong><span class="network-meta">${escapeHtml(network.security || 'secured')}</span></span></label>`).join('') : '<div class="muted">No networks were returned.</div>';
  byId('review').innerHTML = Object.entries({ Network: ssid() || 'Not selected', 'Wi-Fi password': openSecurity() ? 'Not needed' : 'Set', Country: state.wifiCountry || 'Not set', 'SSH mode': state.sshMode, 'SSH public key': state.sshMode === 'key' ? (state.sshPublicKey || 'Missing') : 'Not used', 'SSH password': state.sshMode === 'password' ? 'Set' : 'Not used', Hostname: state.hostname || 'Default' }).map(([label, value]) => `<div class="review-row"><strong>${escapeHtml(label)}</strong><span>${escapeHtml(value)}</span></div>`).join('');
  message();
};

const errorForWifi = () => {
  if (!ssid()) return 'Choose a Wi-Fi network or enter the SSID manually.';
  if (!/^[A-Z]{2}$/.test(state.wifiCountry)) return 'Enter a two-letter Wi-Fi country code.';
  if (!openSecurity() && !state.wifiPassphrase) return 'This network needs a Wi-Fi password.';
  return '';
};

const errorForSsh = () => {
  if (state.sshMode === 'key' && !state.sshPublicKey) return 'Paste an SSH public key or switch to password access.';
  if (state.sshMode === 'password' && (state.sshPassword.length < 12 || state.sshPassword !== state.sshPasswordConfirm)) return 'Use a matching SSH password of at least 12 characters.';
  return '';
};

const loadNetworks = async () => {
  try {
    const response = await fetch('/networks', { cache: 'no-store' });
    const payload = await response.json();
    const list = Array.isArray(payload) ? payload : payload.networks;
    state.networks = (Array.isArray(list) ? list : []).map((network) => ({ ssid: network.ssid ?? network.SSID ?? network.name ?? '', security: String(network.security ?? network.sec ?? '').toLowerCase() })).filter((network) => network.ssid);
    if (!state.selectedSsid) state.selectedSsid = state.networks[0]?.ssid ?? '';
  } catch (_error) {
    state.networks = [];
  }
  render();
};

const next = () => {
  sync();
  const error = state.step === 1 ? errorForWifi() : state.step === 2 ? errorForSsh() : '';
  if (error) { state.error = error; render(); return; }
  state.error = '';
  state.step = Math.min(5, state.step + 1);
  render();
};

const stageAndConnect = async (event) => {
  event.preventDefault();
  sync();
  const error = errorForWifi() || errorForSsh();
  if (error) { state.error = error; state.step = errorForWifi() ? 1 : 2; render(); return; }
  state.busy = true;
  state.status = 'Saving device settings…';
  render();
  const stageBody = { sshMode: state.sshMode, sshPublicKey: state.sshMode === 'key' ? state.sshPublicKey : '', sshPassword: state.sshMode === 'password' ? state.sshPassword : '', sshPasswordConfirm: state.sshMode === 'password' ? state.sshPasswordConfirm : '', hostname: state.hostname, wifiCountry: state.wifiCountry };
  try {
    const staged = await fetch('http://192.168.42.1:8080/stage', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(stageBody) });
    if (!staged.ok) throw new Error('Setup stage failed.');
    const connectBody = new URLSearchParams({ ssid: ssid(), identity: '', passphrase: openSecurity() ? '' : state.wifiPassphrase });
    const connected = await fetch('/connect', { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: connectBody });
    if (!connected.ok) throw new Error('Wi-Fi connection failed.');
    state.step = 5;
    state.status = 'Setup request sent. The hotspot should disappear once the device joins Wi-Fi.';
  } catch (caught) {
    state.error = caught instanceof Error ? caught.message : 'Setup failed.';
    state.status = '';
  } finally {
    state.busy = false;
    render();
  }
};

form.addEventListener('click', (event) => {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  if (target.id === 'startButton') { state.step = 1; render(); }
  if (target.matches('[data-next]')) next();
  if (target.matches('[data-back]')) { state.step = Math.max(0, state.step - 1); render(); }
  if (target.id === 'refreshNetworks') loadNetworks();
});
form.addEventListener('change', (event) => {
  const target = event.target;
  if (target.name === 'ssidChoice') { state.selectedSsid = target.value; state.manualSsid = ''; }
  sync();
  render();
});
form.addEventListener('input', () => { sync(); render(); });
form.addEventListener('submit', stageAndConnect);
render();
loadNetworks();
