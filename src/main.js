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
let waveAnimFrame = null;

// ── DOM Elements ──────────────────────────────────────────────────────────────
const screenOnboarding  = document.getElementById('screen-onboarding');
const screenMain        = document.getElementById('screen-main');
const appHeader         = document.querySelector('.app-header');
const statusBadge       = document.getElementById('status-badge');
const statusLabel       = document.getElementById('status-label');
const deviceSelect      = document.getElementById('device-select');
const pitchSlider       = document.getElementById('pitch-slider');
const pitchValueDisplay = document.getElementById('pitch-value-display');
const activeCard        = document.getElementById('active-card');
const btnToggle         = document.getElementById('btn-toggle');
const btnToggleLabel    = document.getElementById('btn-toggle-label');
const btnIcon           = btnToggle.querySelector('.btn-icon');
const outputDeviceName  = document.getElementById('output-device-name');
const waveformBars      = document.getElementById('waveform-bars');
const waveformBarEls    = waveformBars ? Array.from(waveformBars.querySelectorAll('span')) : [];
const btnCheckBlackhole = document.getElementById('btn-check-blackhole');
const btnCopyBrew       = document.getElementById('btn-copy-brew');
const presetBtns        = document.querySelectorAll('.preset-btn');
const previewToggle     = document.getElementById('preview-toggle');
const btnStyleMale       = document.getElementById('btn-style-male');
const btnStyleFemale     = document.getElementById('btn-style-female');
const sliderLabelLeft    = document.getElementById('slider-label-left');
const sliderLabelRight   = document.getElementById('slider-label-right');
const usageTimeLeft     = document.getElementById('usage-time-left');
const limitCard         = document.getElementById('limit-card');
const meetSelectHint    = document.getElementById('meet-select-hint');

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
  try {
    const blackholeOk = await invoke('check_blackhole');
    if (!blackholeOk) { showScreen('onboarding'); return; }
    showScreen('main');
    await loadDevices();
    startWaveformAnimation();
    
    await checkUsageStatus();
    await listen('free-limit-reached', () => {
      checkUsageStatus();
    });
    await listen('auto-started', () => {
      isRunning = true;
      setStatus('active', 'Active');
      setToggleState('stop');
      activeCard.classList.remove('hidden');
      deviceSelect.disabled = true;
      startVisualizer();
      checkUsageStatus();
    });
    await listen('auto-stopped', () => {
      isRunning = false;
      setStatus('idle', 'Idle');
      setToggleState('start');
      activeCard.classList.add('hidden');
      deviceSelect.disabled = false;
      stopVisualizer();
      checkUsageStatus();
    });
    await listen('pitch-changed-from-tray', (event) => {
      const semitones = parseFloat(event.payload);
      if (!isNaN(semitones)) {
        pitchSlider.value = semitones;
        updatePitchDisplay(semitones);
      }
    });
    setInterval(checkUsageStatus, 5000);
  } catch (err) {
    console.error('Init error:', err);
    showScreen('main');
    await loadDevices();
    startWaveformAnimation();
    
    checkUsageStatus().catch(console.error);
    listen('free-limit-reached', () => {
      checkUsageStatus();
    }).catch(console.error);
    listen('auto-started', () => {
      isRunning = true;
      setStatus('active', 'Active');
      setToggleState('stop');
      activeCard.classList.remove('hidden');
      deviceSelect.disabled = true;
      startVisualizer();
      checkUsageStatus();
    }).catch(console.error);
    listen('auto-stopped', () => {
      isRunning = false;
      setStatus('idle', 'Idle');
      setToggleState('start');
      activeCard.classList.add('hidden');
      deviceSelect.disabled = false;
      stopVisualizer();
      checkUsageStatus();
    }).catch(console.error);
    listen('pitch-changed-from-tray', (event) => {
      const semitones = parseFloat(event.payload);
      if (!isNaN(semitones)) {
        pitchSlider.value = semitones;
        updatePitchDisplay(semitones);
      }
    }).catch(console.error);
    setInterval(checkUsageStatus, 5000);
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

const usageBadge        = document.getElementById('usage-badge');

function updateUsageUI(status) {
  usageBadge?.classList.add('hidden');
  limitCard?.classList.add('hidden');
}

// ── Screen Management ─────────────────────────────────────────────────────────
function showScreen(name) {
  screenOnboarding.classList.add('hidden');
  screenMain.classList.add('hidden');
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
    screenOnboarding.classList.remove('hidden');
  } else {
    screenMain.classList.remove('hidden');
  }
}

// ── Onboarding ────────────────────────────────────────────────────────────────
const btnInstallDriver = document.getElementById('btn-install-driver');
const installStatusText = document.getElementById('install-status-text');

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
          startWaveformAnimation();
        } else {
          btnInstallDriver.disabled = false;
          btnInstallDriver.textContent = '⚡ Install Audio Bridge (1-Click)';
          if (installStatusText) installStatusText.textContent = 'Installed! Please restart your computer to activate.';
        }
      }
    }, 1200);
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
    if (ok) { showScreen('main'); await loadDevices(); startWaveformAnimation(); }
    else {
      const isWindows = /Win/i.test(navigator.userAgent || navigator.platform);
      btnCheckBlackhole.textContent = isWindows ? '✗ Not detected — restart your PC first' : '✗ Not detected — restart your Mac first';
      setTimeout(() => { btnCheckBlackhole.textContent = '✓ I\'ve Already Installed It — Check Now'; btnCheckBlackhole.disabled = false; }, 3000);
    }
  } catch { btnCheckBlackhole.textContent = '✓ I\'ve Already Installed It — Check Now'; btnCheckBlackhole.disabled = false; }
});

btnCopyBrew?.addEventListener('click', () => {
  navigator.clipboard.writeText('brew install blackhole-2ch').then(() => {
    btnCopyBrew.textContent = '✅';
    setTimeout(() => { btnCopyBrew.textContent = '📋'; }, 2000);
  });
});

// ── Window Dragging ───────────────────────────────────────────────────────────
appHeader?.addEventListener('mousedown', (e) => {
  if (e.button === 0) { e.preventDefault(); appWindow.startDragging(); }
});

// ── Device Loading ────────────────────────────────────────────────────────────
async function loadDevices() {
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

deviceSelect.addEventListener('change', () => { selectedDevice = deviceSelect.value; });

// ── Voice Style Selection (Masculine / Feminine) ──────────────────────────────
let currentMode = 'male';

btnStyleMale?.addEventListener('click', () => {
  if (currentMode === 'male') return;
  setVoiceMode('male');
});

btnStyleFemale?.addEventListener('click', () => {
  if (currentMode === 'female') return;
  setVoiceMode('female');
});

function setVoiceMode(mode) {
  currentMode = mode;
  btnStyleMale?.classList.toggle('active', mode === 'male');
  btnStyleFemale?.classList.toggle('active', mode === 'female');

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

// ── Pitch Slider ──────────────────────────────────────────────────────────────
function updatePitchDisplay(value) {
  const semitones = parseFloat(value);
  currentPitch = semitones;
  pitchValueDisplay.textContent = `${semitones} st`;
  presetBtns.forEach(btn => btn.classList.toggle('active', parseFloat(btn.dataset.semitones) === semitones));
}

pitchSlider.addEventListener('input', async (e) => {
  updatePitchDisplay(e.target.value);
  try { await invoke('set_pitch', { semitones: parseFloat(e.target.value) }); }
  catch (err) { console.error('Failed to update pitch:', err); }
});

presetBtns.forEach(btn => {
  btn.addEventListener('click', async () => {
    const semitones = parseFloat(btn.dataset.semitones);
    pitchSlider.value = semitones;
    updatePitchDisplay(semitones);
    try { await invoke('set_pitch', { semitones }); }
    catch (err) { console.error('Failed to update pitch from preset:', err); }
  });
});

updatePitchDisplay(pitchSlider.value);

previewToggle?.addEventListener('change', async (e) => {
  const enabled = e.target.checked;
  try {
    await invoke('toggle_preview', { enabled });
  } catch (err) {
    console.error('Failed to toggle preview:', err);
    e.target.checked = !enabled;
  }
});

// ── Toggle Button ─────────────────────────────────────────────────────────────
btnToggle.addEventListener('click', async () => {
  if (!isRunning) await startProcessing();
  else await stopProcessing();
  await checkUsageStatus().catch(console.error);
});

async function startProcessing() {
  if (!selectedDevice) { setStatus('error', 'No mic selected'); return; }
  setStatus('loading', 'Starting...');
  btnToggle.disabled = true;
  try {
    await invoke('start_processing', { inputDevice: selectedDevice, pitchSemitones: currentPitch });
    isRunning = true;
    setStatus('active', 'Active');
    setToggleState('stop');
    activeCard.classList.remove('hidden');
    deviceSelect.disabled = true;
    
    if (previewToggle) {
      try { await invoke('toggle_preview', { enabled: previewToggle.checked }); } catch (_) {}
    }
    
    startVisualizer();
  } catch (err) {
    console.error('Failed to start:', err);
    setStatus('error', 'Error');
    showError(err);
  } finally {
    btnToggle.disabled = false;
  }
}

async function stopProcessing() {
  setStatus('loading', 'Stopping...');
  btnToggle.disabled = true;
  try {
    await invoke('stop_processing');
    isRunning = false;
    setStatus('idle', 'Idle');
    setToggleState('start');
    activeCard.classList.add('hidden');
    deviceSelect.disabled = false;
    stopVisualizer();
  } catch (err) {
    console.error('Failed to stop:', err);
    setStatus('error', 'Error');
  } finally {
    btnToggle.disabled = false;
  }
}

// ── Status & Toggle Helpers ───────────────────────────────────────────────────
function setStatus(type, label) {
  statusBadge.className = 'status-badge';
  statusBadge.classList.add(`status-${type}`);
  statusLabel.textContent = label;
}

function setToggleState(state) {
  if (state === 'stop') {
    btnToggle.className = 'btn-toggle btn-stop';
    btnIcon.textContent = '■';
    btnToggleLabel.textContent = 'Stop MicMorph';
  } else {
    btnToggle.className = 'btn-toggle btn-start';
    btnIcon.textContent = '▶';
    btnToggleLabel.textContent = 'Start MicMorph';
  }
}

function showError(err) {
  const msg = typeof err === 'string' ? err : JSON.stringify(err);
  statusLabel.textContent = msg.length > 30 ? msg.slice(0, 30) + '...' : msg;
  setTimeout(() => setStatus('idle', 'Idle'), 4000);
}

// ── Waveform (CSS bars — no canvas, guaranteed to render) ─────────────────────
const BAR_COUNT = waveformBarEls.length || 30;
let wavePhase = 0;
let latestRms = 0;
let pollInterval = null;

// Poll Rust every 50ms for audio level — no events, no permissions needed
function startVisualizer() {
  if (pollInterval) clearInterval(pollInterval);
  pollInterval = setInterval(async () => {
    try { latestRms = await invoke('get_audio_level'); } catch (_) {}
  }, 50);
}

function stopVisualizer() {
  if (pollInterval) { clearInterval(pollInterval); pollInterval = null; }
  latestRms = 0;
}

function startWaveformAnimation() {
  if (waveAnimFrame) cancelAnimationFrame(waveAnimFrame);
  const bars = waveformBarEls;
  const count = bars.length;
  if (count === 0) return; // safety guard

  function drawFrame() {
    const maxH = 50;
    const minH = 6; // visible as bars even at idle
    let rmsVal = parseFloat(latestRms) || 0;
    if (rmsVal < 0) rmsVal = 0;

    for (let i = 0; i < count; i++) {
      const sine = Math.sin(wavePhase + (i / count) * Math.PI * 2) * 0.5 + 0.5;
      const boost = isRunning ? Math.min(rmsVal * 400, 40) : 0;
      // idle: bars range from 6px to 20px, active: 6px to 50px driven by voice
      const h = minH + sine * (isRunning ? 20 + boost : 14);
      bars[i].style.height = Math.min(h, maxH) + 'px';
    }
    waveformBars.classList.toggle('active', isRunning);
    wavePhase += isRunning ? 0.10 : 0.02;
    waveAnimFrame = requestAnimationFrame(drawFrame);
  }
  drawFrame();
}

// ── Keyboard shortcut ─────────────────────────────────────────────────────────
document.addEventListener('keydown', (e) => {
  if (e.code === 'Space' && e.target === document.body) {
    e.preventDefault();
    btnToggle.click();
  }
});

// ── Boot ──────────────────────────────────────────────────────────────────────
init();
