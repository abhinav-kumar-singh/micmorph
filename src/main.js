// main.js — MicMorph UI Logic
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';

const appWindow = getCurrentWindow();
window.invoke = invoke;

// ── State ─────────────────────────────────────────────────────────────────────
let isRunning = false;
let currentPitch = -3;
let selectedDevice = '';
let currentMode = 'male';

// ── DOM Elements (initialized in initDOM) ─────────────────────────────────────
let screenOnboarding, screenMain, appHeader, statusBadge, statusLabel;
let deviceSelect, pitchSlider, pitchValueDisplay, activeCard;
let btnToggle, btnToggleLabel, btnIcon, outputDeviceName;
let waveformBars, waveformBarEls = [];
let btnCheckBlackhole, btnCopyBrew, presetBtns = [];
let previewToggle, btnStyleMale, btnStyleFemale;
let sliderLabelLeft, sliderLabelRight, usageTimeLeft, limitCard;
let meetSelectHint, usageBadge, btnInstallDriver, installStatusText;

// ── Anonymous Telemetry (PostHog) ─────────────────────────────────────────────
async function trackEvent(eventName, properties = {}) {
  const key = import.meta.env?.VITE_POSTHOG_KEY || 'phc_oqYPUzwrXfhq9iZTVbBAh4md574jZfVtsjLf88N2hrLf';
  const host = import.meta.env?.VITE_POSTHOG_HOST || 'https://us.i.posthog.com';
  if (!key) return;

  try {
    let distinctId = localStorage.getItem('mm_anon_user_id');
    if (!distinctId) {
      distinctId = 'anon_' + Math.random().toString(36).substring(2, 12) + '_' + Date.now().toString(36);
      localStorage.setItem('mm_anon_user_id', distinctId);
    }

    const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
    await fetch(`${host.replace(/\/$/, '')}/capture/`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        api_key: key,
        event: eventName,
        distinct_id: distinctId,
        properties: {
          $os: isWindows ? 'Windows' : 'macOS',
          app_version: '0.1.0',
          ...properties,
        },
      }),
    });
  } catch {
    // Fail silently — analytics must never disrupt app operations
  }
}

function updateOutputDeviceLabels() {
  const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
  if (isWindows) {
    if (outputDeviceName) outputDeviceName.textContent = 'CABLE Output (VB-Audio)';
    if (meetSelectHint) meetSelectHint.innerHTML = '💡 In Google Meet / Zoom, select <strong>"CABLE Output"</strong> as your mic.';
  } else {
    if (outputDeviceName) outputDeviceName.textContent = 'MicMorph (BlackHole)';
    if (meetSelectHint) meetSelectHint.innerHTML = '💡 In Google Meet / Zoom, select <strong>"MicMorph"</strong> (or BlackHole) as your mic.';
  }
}

// ── Init ──────────────────────────────────────────────────────────────────────
async function init() {
  updateOutputDeviceLabels();
  trackEvent('app_opened');
  try {
    const blackholeOk = await invoke('check_blackhole');
    if (!blackholeOk) { showScreen('onboarding'); return; }
    showScreen('main');
    await loadDevices();
    
    await checkUsageStatus();
    await listen('free-limit-reached', () => {
      checkUsageStatus();
    });
    await listen('pitch-changed-from-tray', (event) => {
      const semitones = parseFloat(event.payload);
      if (!isNaN(semitones)) {
        if (pitchSlider) pitchSlider.value = semitones;
        updatePitchDisplay(semitones);
      }
    });
    setInterval(checkUsageStatus, 15000);
  } catch (err) {
    console.error('Init error:', err);
    showScreen('main');
    await loadDevices();
    checkUsageStatus().catch(console.error);
    listen('pitch-changed-from-tray', (event) => {
      const semitones = parseFloat(event.payload);
      if (!isNaN(semitones)) {
        if (pitchSlider) pitchSlider.value = semitones;
        updatePitchDisplay(semitones);
      }
    }).catch(console.error);
  }
}

async function checkUsageStatus() {
  try {
    const status = await invoke('get_usage_status');
    updateUsageUI(status);
  } catch (err) {
    console.error('Failed to get usage status:', err);
  }
}

function updateUsageUI(status) {
  usageBadge?.classList.add('hidden');
  limitCard?.classList.add('hidden');
}

// ── Screen Management ─────────────────────────────────────────────────────────
function showScreen(name) {
  screenOnboarding?.classList.add('hidden');
  screenMain?.classList.add('hidden');
  if (name === 'onboarding') {
    const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
    const step1Title = document.getElementById('step1-title');
    const step1Desc = document.getElementById('step1-desc');
    const driverAction = document.getElementById('driver-action-container');
    const step2Title = document.getElementById('step2-title');
    const step2Desc = document.getElementById('step2-desc');
    const step3Title = document.getElementById('step3-title');
    const step3Desc = document.getElementById('step3-desc');

    if (isWindows) {
      if (step1Title) step1Title.textContent = 'Install VB-Audio Virtual Cable';
      if (step1Desc) step1Desc.textContent = 'VB-Audio Virtual Cable is a free, trusted audio driver for Windows. MicMorph uses it to send your voice to Zoom, Meet, Teams, and Discord.';
      if (driverAction) {
        driverAction.innerHTML = `<a href="https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip" target="_blank" style="padding:8px 16px; font-size:0.85rem; text-decoration:none; display:inline-flex; align-items:center; gap:6px; color:var(--accent); font-weight:600; border:1px solid var(--border-card); border-radius:8px; background:var(--bg-card);">📥 Download Driver Installer (ZIP)</a>`;
      }
      if (step2Title) step2Title.textContent = 'Restart Your Device';
      if (step2Desc) step2Desc.textContent = 'After installation, restart your computer once to register the virtual audio driver.';
      if (step3Title) step3Title.textContent = 'Return to MicMorph';
      if (step3Desc) step3Desc.textContent = 'Launch MicMorph — your virtual microphone bridge will be ready.';
    } else {
      if (step1Title) step1Title.textContent = 'Install BlackHole Virtual Mic';
      if (step1Desc) step1Desc.textContent = 'BlackHole is a free, trusted virtual audio driver. MicMorph uses it to route your processed voice to Zoom, Meet, and Slack.';
      if (driverAction) {
        driverAction.innerHTML = `<code id="brew-command">brew install blackhole-2ch</code> <button id="btn-copy-brew" class="btn-copy" title="Copy command">📋</button>`;
        document.getElementById('btn-copy-brew')?.addEventListener('click', () => {
          navigator.clipboard.writeText('brew install blackhole-2ch').then(() => {
            const btn = document.getElementById('btn-copy-brew');
            if (btn) { btn.textContent = '✅'; setTimeout(() => { btn.textContent = '📋'; }, 2000); }
          });
        });
      }
      if (step2Title) step2Title.textContent = 'Restart Your Device';
      if (step2Desc) step2Desc.textContent = 'After installation, restart your computer once to activate the virtual audio driver.';
      if (step3Title) step3Title.textContent = 'Return to MicMorph';
      if (step3Desc) step3Desc.textContent = 'Come back and click the button below — we\'ll detect BlackHole automatically.';
    }
    screenOnboarding?.classList.remove('hidden');
  } else {
    screenMain?.classList.remove('hidden');
  }
}

// ── Device Loading ────────────────────────────────────────────────────────────
async function loadDevices() {
  if (!deviceSelect) return;
  try {
    const devices = await invoke('get_input_devices');
    const defaultDevice = await invoke('get_default_input_device');
    deviceSelect.innerHTML = '';
    if (!devices || devices.length === 0) {
      deviceSelect.innerHTML = '<option value="">No microphones found</option>';
      return;
    }
    devices.forEach(device => {
      if (!device.is_blackhole) {
        const option = document.createElement('option');
        option.value = device.name;
        option.textContent = device.name;
        deviceSelect.appendChild(option);
      }
    });
    if (defaultDevice && deviceSelect.querySelector(`option[value="${defaultDevice}"]`)) {
      deviceSelect.value = defaultDevice;
    }
    selectedDevice = deviceSelect.value;
  } catch (err) {
    console.error('Failed to load devices:', err);
    deviceSelect.innerHTML = '<option value="">Error loading devices</option>';
  }
}

// ── Voice Style Selection (Masculine / Feminine) ──────────────────────────────
function setVoiceMode(mode) {
  currentMode = mode;
  btnStyleMale?.classList.toggle('active', mode === 'male');
  btnStyleFemale?.classList.toggle('active', mode === 'female');

  if (!pitchSlider) return;

  if (mode === 'male') {
    pitchSlider.min = '-8';
    pitchSlider.max = '-1';
    pitchSlider.value = '-3';
    if (sliderLabelRight) sliderLabelRight.textContent = 'Deepest';
    
    updatePresets([
      { label: 'Subtle', val: -1 },
      { label: 'Medium', val: -3 },
      { label: 'Deep', val: -5 },
      { label: 'Deepest', val: -8 }
    ]);
    
    updatePitchDisplay(-3);
    sendPitchChange(-3);
  } else {
    pitchSlider.min = '1';
    pitchSlider.max = '8';
    pitchSlider.value = '4';
    if (sliderLabelRight) sliderLabelRight.textContent = 'Highest';
    
    updatePresets([
      { label: 'Subtle', val: 1.5 },
      { label: 'Medium', val: 3.5 },
      { label: 'High', val: 5.5 },
      { label: 'Highest', val: 8 }
    ]);
    
    updatePitchDisplay(4);
    sendPitchChange(4);
  }
}

function updatePresets(presets) {
  const buttons = document.querySelectorAll('.preset-btn');
  presets.forEach((p, i) => {
    if (buttons[i]) {
      buttons[i].dataset.semitones = p.val;
      buttons[i].textContent = p.label;
    }
  });
}

async function sendPitchChange(val) {
  try {
    await invoke('set_pitch', { semitones: parseFloat(val) });
  } catch (err) {
    console.error('Failed to update pitch:', err);
  }
}

// ── Pitch Display ─────────────────────────────────────────────────────────────
function updatePitchDisplay(value) {
  const semitones = parseFloat(value);
  currentPitch = semitones;
  if (pitchValueDisplay) pitchValueDisplay.textContent = `${semitones} st`;
  presetBtns.forEach(btn => btn?.classList?.toggle('active', parseFloat(btn.dataset?.semitones) === semitones));
}

// ── Processing Control ────────────────────────────────────────────────────────
async function startProcessing() {
  if (!selectedDevice) { setStatus('error', 'No mic selected'); return; }
  setStatus('loading', 'Starting...');
  if (btnToggle) btnToggle.disabled = true;
  try {
    await invoke('start_processing', { inputDevice: selectedDevice, pitchSemitones: currentPitch });
    isRunning = true;
    setStatus('active', 'Active');
    setToggleState('stop');
    activeCard?.classList.remove('hidden');
    if (deviceSelect) deviceSelect.disabled = true;
    trackEvent('morph_started', { pitch: currentPitch });
    
    if (previewToggle) {
      try { await invoke('toggle_preview', { enabled: previewToggle.checked }); } catch (_) {}
    }
    
    startVisualizer();
  } catch (err) {
    console.error('Failed to start:', err);
    setStatus('error', 'Error');
    showError(err);
  } finally {
    if (btnToggle) btnToggle.disabled = false;
  }
}

async function stopProcessing() {
  setStatus('loading', 'Stopping...');
  if (btnToggle) btnToggle.disabled = true;
  try {
    await invoke('stop_processing');
    isRunning = false;
    setStatus('idle', 'Idle');
    setToggleState('start');
    activeCard?.classList.add('hidden');
    if (deviceSelect) deviceSelect.disabled = false;
    stopVisualizer();
  } catch (err) {
    console.error('Failed to stop:', err);
    setStatus('error', 'Error');
  } finally {
    if (btnToggle) btnToggle.disabled = false;
  }
}

// ── Status & Toggle Helpers ───────────────────────────────────────────────────
function setStatus(type, label) {
  if (!statusBadge || !statusLabel) return;
  statusBadge.className = 'status-badge';
  statusBadge.classList.add(`status-${type}`);
  statusLabel.textContent = label;
}

function setToggleState(state) {
  if (!btnToggle || !btnToggleLabel) return;
  if (state === 'stop') {
    btnToggle.className = 'btn-toggle btn-stop';
    if (btnIcon) btnIcon.textContent = '■';
    btnToggleLabel.textContent = 'Stop MicMorph';
  } else {
    btnToggle.className = 'btn-toggle btn-start';
    if (btnIcon) btnIcon.textContent = '▶';
    btnToggleLabel.textContent = 'Start MicMorph';
  }
}

function showError(err) {
  if (!statusLabel) return;
  const msg = typeof err === 'string' ? err : JSON.stringify(err);
  statusLabel.textContent = msg.length > 30 ? msg.slice(0, 30) + '...' : msg;
  setTimeout(() => setStatus('idle', 'Idle'), 4000);
}

// ── Waveform Visualizer (GPU-accelerated, 0% CPU when idle) ──────────────────
let wavePhase = 0;
let latestRms = 0;
let pollInterval = null;
let visualizerFrame = null;

function startVisualizer() {
  if (pollInterval) clearInterval(pollInterval);
  if (visualizerFrame) cancelAnimationFrame(visualizerFrame);
  waveformBars?.classList.add('active');

  // Poll Rust every 80ms for audio level
  pollInterval = setInterval(async () => {
    try { 
      if (isRunning) latestRms = await invoke('get_audio_level'); 
    } catch (_) {}
  }, 80);

  const bars = waveformBarEls;
  const count = bars.length;
  if (count === 0) return;

  function renderActiveFrame() {
    if (!isRunning) return;
    let rmsVal = parseFloat(latestRms) || 0;
    if (rmsVal < 0) rmsVal = 0;

    for (let i = 0; i < count; i++) {
      const sine = Math.sin(wavePhase + (i / count) * Math.PI * 2) * 0.5 + 0.5;
      const boost = Math.min(rmsVal * 350, 30);
      const scale = 1.0 + (sine * 1.5) + (boost * 0.25);
      bars[i].style.transform = `scaleY(${Math.min(scale, 7.0).toFixed(2)})`;
    }
    wavePhase += 0.12;
    visualizerFrame = requestAnimationFrame(renderActiveFrame);
  }
  renderActiveFrame();
}

function stopVisualizer() {
  if (pollInterval) { clearInterval(pollInterval); pollInterval = null; }
  if (visualizerFrame) { cancelAnimationFrame(visualizerFrame); visualizerFrame = null; }
  waveformBars?.classList.remove('active');
  latestRms = 0;
  waveformBarEls.forEach(b => {
    b.style.transform = 'scaleY(1)';
  });
}

// ── Initialize DOM References ────────────────────────────────────────────────
function initDOM() {
  screenOnboarding  = document.getElementById('screen-onboarding');
  screenMain        = document.getElementById('screen-main');
  appHeader         = document.querySelector('.app-header');
  statusBadge       = document.getElementById('status-badge');
  statusLabel       = document.getElementById('status-label');
  deviceSelect      = document.getElementById('device-select');
  pitchSlider       = document.getElementById('pitch-slider');
  pitchValueDisplay = document.getElementById('pitch-value-display');
  activeCard        = document.getElementById('active-card');
  btnToggle         = document.getElementById('btn-toggle');
  btnToggleLabel    = document.getElementById('btn-toggle-label');
  btnIcon           = btnToggle?.querySelector('.btn-icon');
  outputDeviceName  = document.getElementById('output-device-name');
  waveformBars      = document.getElementById('waveform-bars');
  waveformBarEls    = waveformBars ? Array.from(waveformBars.querySelectorAll('span')) : [];
  btnCheckBlackhole = document.getElementById('btn-check-blackhole');
  btnCopyBrew       = document.getElementById('btn-copy-brew');
  presetBtns        = Array.from(document.querySelectorAll('.preset-btn'));
  previewToggle     = document.getElementById('preview-toggle');
  btnStyleMale      = document.getElementById('btn-style-male');
  btnStyleFemale    = document.getElementById('btn-style-female');
  sliderLabelLeft   = document.getElementById('slider-label-left');
  sliderLabelRight  = document.getElementById('slider-label-right');
  usageTimeLeft     = document.getElementById('usage-time-left');
  limitCard         = document.getElementById('limit-card');
  meetSelectHint    = document.getElementById('meet-select-hint');
  usageBadge        = document.getElementById('usage-badge');
  btnInstallDriver  = document.getElementById('btn-install-driver');
  installStatusText = document.getElementById('install-status-text');
}

// ── Attach Event Listeners Safely ─────────────────────────────────────────────
function attachEventListeners() {
  btnInstallDriver?.addEventListener('click', async () => {
    btnInstallDriver.disabled = true;
    btnInstallDriver.textContent = '⏳ Configuring Audio Bridge...';
    if (installStatusText) {
      const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
      installStatusText.textContent = isWindows 
        ? 'Please click "Yes" on the Windows administrator prompt...' 
        : 'Please confirm with Touch ID or enter your Mac password...';
    }

    try {
      const res = await invoke('install_virtual_driver');
      console.log('Install result:', res);
      if (installStatusText) installStatusText.textContent = '✓ Driver installed! Detecting audio bridge...';
      
      let attempts = 0;
      const interval = setInterval(async () => {
        attempts++;
        const ok = await invoke('check_blackhole');
        if (ok || attempts > 8) {
          clearInterval(interval);
          if (ok) {
            showScreen('main');
            await loadDevices();
          } else {
            btnInstallDriver.disabled = false;
            btnInstallDriver.textContent = '⚡ Install Audio Bridge (1-Click)';
            if (installStatusText) installStatusText.textContent = 'Installed! Please restart your computer to activate.';
          }
        }
      }, 1000);
    } catch (err) {
      console.error('Driver install error:', err);
      btnInstallDriver.disabled = false;
      btnInstallDriver.textContent = '⚡ Try Again (1-Click Install)';
      if (installStatusText) installStatusText.textContent = 'Installation was canceled or failed. You can also use manual setup below.';
    }
  });

  btnCheckBlackhole?.addEventListener('click', async () => {
    btnCheckBlackhole.textContent = 'Checking...';
    btnCheckBlackhole.disabled = true;
    try {
      const ok = await invoke('check_blackhole');
      if (ok) { 
        showScreen('main'); 
        await loadDevices(); 
      } else {
        const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
        btnCheckBlackhole.textContent = isWindows ? '✗ Not detected — restart your PC first' : '✗ Not detected — restart your Mac first';
        setTimeout(() => { 
          if (btnCheckBlackhole) {
            btnCheckBlackhole.textContent = '✓ I\'ve Already Installed It — Check Now'; 
            btnCheckBlackhole.disabled = false; 
          }
        }, 3000);
      }
    } catch { 
      if (btnCheckBlackhole) {
        btnCheckBlackhole.textContent = '✓ I\'ve Already Installed It — Check Now'; 
        btnCheckBlackhole.disabled = false; 
      }
    }
  });

  btnCopyBrew?.addEventListener('click', () => {
    navigator.clipboard.writeText('brew install blackhole-2ch').then(() => {
      if (btnCopyBrew) {
        btnCopyBrew.textContent = '✅';
        setTimeout(() => { if (btnCopyBrew) btnCopyBrew.textContent = '📋'; }, 2000);
      }
    });
  });

  appHeader?.addEventListener('mousedown', (e) => {
    if (e.button === 0) { e.preventDefault(); appWindow.startDragging(); }
  });

  deviceSelect?.addEventListener('change', () => { 
    if (deviceSelect) selectedDevice = deviceSelect.value; 
  });

  btnStyleMale?.addEventListener('click', () => {
    if (currentMode === 'male') return;
    setVoiceMode('male');
  });

  btnStyleFemale?.addEventListener('click', () => {
    if (currentMode === 'female') return;
    setVoiceMode('female');
  });

  pitchSlider?.addEventListener('input', async (e) => {
    updatePitchDisplay(e.target.value);
    try { await invoke('set_pitch', { semitones: parseFloat(e.target.value) }); }
    catch (err) { console.error('Failed to update pitch:', err); }
  });

  presetBtns.forEach(btn => {
    btn?.addEventListener('click', async () => {
      const semitones = parseFloat(btn.dataset.semitones);
      if (pitchSlider) pitchSlider.value = semitones;
      updatePitchDisplay(semitones);
      try { await invoke('set_pitch', { semitones }); }
      catch (err) { console.error('Failed to update pitch from preset:', err); }
    });
  });

  if (pitchSlider) {
    updatePitchDisplay(pitchSlider.value);
  }

  previewToggle?.addEventListener('change', async (e) => {
    const enabled = e.target.checked;
    try {
      await invoke('toggle_preview', { enabled });
    } catch (err) {
      console.error('Failed to toggle preview:', err);
      e.target.checked = !enabled;
    }
  });

  btnToggle?.addEventListener('click', async () => {
    if (!isRunning) await startProcessing();
    else await stopProcessing();
    await checkUsageStatus().catch(console.error);
  });

  document.addEventListener('keydown', (e) => {
    if (e.code === 'Space' && e.target === document.body) {
      e.preventDefault();
      btnToggle?.click();
    }
  });
}

// ── Safe Boot Lifecycle ───────────────────────────────────────────────────────
function startApp() {
  try {
    initDOM();
    attachEventListeners();
    init();
  } catch (err) {
    console.error('Fatal initialization error:', err);
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', startApp);
} else {
  startApp();
}
