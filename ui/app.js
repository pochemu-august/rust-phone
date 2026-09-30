// RustPhone Softphone Core Client
// Dual Web Audio API synthesis for realistic DTMF tone generation & Tauri IPC

const DTMF_FREQS = {
  '1': [697, 1209], '2': [697, 1336], '3': [697, 1477],
  '4': [770, 1209], '5': [770, 1336], '6': [770, 1477],
  '7': [852, 1209], '8': [852, 1336], '9': [852, 1477],
  '*': [941, 1209], '0': [941, 1336], '#': [941, 1477],
};

let audioCtx = null;
function getAudioContext() {
  if (!audioCtx) {
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (AudioContextClass) {
      audioCtx = new AudioContextClass();
    }
  }
  if (audioCtx && audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
}

function playDtmfTone(digit, durationMs = 120) {
  const freqs = DTMF_FREQS[digit];
  if (!freqs) return;
  const ctx = getAudioContext();
  if (!ctx) return;

  const osc1 = ctx.createOscillator();
  const osc2 = ctx.createOscillator();
  const gain = ctx.createGain();

  osc1.frequency.value = freqs[0];
  osc2.frequency.value = freqs[1];

  gain.gain.setValueAtTime(0.08, ctx.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + (durationMs / 1000));

  osc1.connect(gain);
  osc2.connect(gain);
  gain.connect(ctx.destination);

  osc1.start();
  osc2.start();
  osc1.stop(ctx.currentTime + (durationMs / 1000));
  osc2.stop(ctx.currentTime + (durationMs / 1000));
}

// Tauri IPC Abstraction
const isTauri = typeof window.__TAURI__ !== 'undefined';
const invoke = isTauri && window.__TAURI__.core ? window.__TAURI__.core.invoke : async (cmd, args) => {
  console.log(`[Mock Tauri Invoke] ${cmd}`, args);
  if (cmd === 'init_sip_stack') {
    return { sip_address: 'sip:local@127.0.0.1:5060', is_initialized: true };
  }
  if (cmd === 'get_audio_devices') {
    return [
      [{ name: 'Default Microphone (Realtek HD Audio)', is_default: true }],
      [{ name: 'Default Speakers (Realtek HD Audio)', is_default: true }]
    ];
  }
  if (cmd === 'make_call') {
    return 'call-' + Math.random().toString(36).substring(2, 9);
  }
  return null;
};

// Application State
const state = {
  currentCallId: null,
  callStartTime: null,
  callTimerInterval: null,
  isMuted: false,
  callHistory: JSON.parse(localStorage.getItem('rustphone_history') || '[]'),
};

// UI Elements
const dialInput = document.getElementById('dial-input');
const btnBackspace = document.getElementById('btn-backspace');
const btnCall = document.getElementById('btn-call');
const statusPill = document.getElementById('status-pill');
const statusText = document.getElementById('status-text');
const endpointAddrDisplay = document.getElementById('endpoint-addr-display');
const historyList = document.getElementById('history-list');
const btnClearHistory = document.getElementById('btn-clear-history');

// Overlays & Modals
const callOverlay = document.getElementById('active-call-overlay');
const activeCallerName = document.getElementById('active-caller-name');
const activeCallerUri = document.getElementById('active-caller-uri');
const activeCallTimer = document.getElementById('active-call-timer');
const btnHangup = document.getElementById('btn-hangup');
const btnToggleMute = document.getElementById('btn-toggle-mute');
const btnToggleKeypad = document.getElementById('btn-toggle-keypad');
const incallKeypad = document.getElementById('incall-keypad');

const incomingModal = document.getElementById('incoming-modal');
const incomingCallerId = document.getElementById('incoming-caller-id');
const incomingCallerUri = document.getElementById('incoming-caller-uri');
const btnIncomingAccept = document.getElementById('btn-incoming-accept');
const btnIncomingReject = document.getElementById('btn-incoming-reject');

// Navigation Tabs
document.querySelectorAll('.nav-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.tab-view').forEach(v => v.classList.remove('active'));

    btn.classList.add('active');
    const tabName = btn.dataset.tab;
    const targetView = document.getElementById(`view-${tabName}`);
    if (targetView) targetView.classList.add('active');
  });
});

// Keypad DTMF & Input
document.querySelectorAll('.key-btn').forEach(btn => {
  btn.addEventListener('click', () => {
    const key = btn.dataset.key;
    playDtmfTone(key);
    dialInput.value += key;
  });
});

// In-call Mini DTMF Keypad
document.querySelectorAll('.mini-key').forEach(btn => {
  btn.addEventListener('click', async () => {
    const digit = btn.dataset.dtmf;
    playDtmfTone(digit);
    if (state.currentCallId) {
      try {
        await invoke('send_dtmf', { callId: state.currentCallId, digit });
      } catch (e) {
        console.error('Failed to send DTMF:', e);
      }
    }
  });
});

btnBackspace.addEventListener('click', () => {
  dialInput.value = dialInput.value.slice(0, -1);
});

// Speed Dial / Quick Call in Contacts
document.querySelectorAll('.contact-card').forEach(card => {
  const callBtn = card.querySelector('.quick-call-btn');
  const uri = card.dataset.uri;
  if (callBtn && uri) {
    callBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      dialInput.value = uri;
      startCall(uri);
    });
  }
});

// Call Actions
async function startCall(destUri) {
  if (!destUri) return;
  try {
    const callId = await invoke('make_call', { destination: destUri, accountId: 0 });
    state.currentCallId = callId;
    showActiveCall(destUri, destUri, 'Calling...');
    saveHistoryItem({ uri: destUri, type: 'outgoing', time: new Date().toISOString() });
  } catch (err) {
    alert('Call failed: ' + err);
  }
}

btnCall.addEventListener('click', () => {
  const dest = dialInput.value.trim();
  if (dest) {
    startCall(dest);
  }
});

btnHangup.addEventListener('click', async () => {
  if (state.currentCallId) {
    try {
      await invoke('hangup_call', { callId: state.currentCallId });
    } catch (e) {
      console.error('Hangup error:', e);
    }
  }
  endActiveCall();
});

btnToggleMute.addEventListener('click', async () => {
  if (!state.currentCallId) return;
  state.isMuted = !state.isMuted;
  try {
    await invoke('set_mute', { callId: state.currentCallId, muteMic: state.isMuted });
    btnToggleMute.classList.toggle('active', state.isMuted);
  } catch (e) {
    console.error('Mute error:', e);
  }
});

btnToggleKeypad.addEventListener('click', () => {
  incallKeypad.classList.toggle('hidden');
});

// Incoming Call Handlers
let incomingCallId = null;

function showIncomingCall(callId, fromUri) {
  incomingCallId = callId;
  incomingCallerId.innerText = fromUri.split('@')[0].replace('sip:', '') || 'Unknown';
  incomingCallerUri.innerText = fromUri;
  incomingModal.classList.remove('hidden');
}

btnIncomingAccept.addEventListener('click', async () => {
  if (!incomingCallId) return;
  const id = incomingCallId;
  incomingModal.classList.add('hidden');
  try {
    await invoke('answer_call', { callId: id, statusCode: 200 });
    state.currentCallId = id;
    showActiveCall('Incoming Call', incomingCallerUri.innerText, 'Connected');
    startCallTimer();
  } catch (e) {
    alert('Failed to answer call: ' + e);
  }
});

btnIncomingReject.addEventListener('click', async () => {
  if (!incomingCallId) return;
  incomingModal.classList.add('hidden');
  try {
    await invoke('hangup_call', { callId: incomingCallId });
  } catch (e) {
    console.error(e);
  }
  incomingCallId = null;
});

// Call UI Helpers
function showActiveCall(name, uri, status) {
  activeCallerName.innerText = name;
  activeCallerUri.innerText = uri;
  activeCallTimer.innerText = status;
  callOverlay.classList.remove('hidden');
}

function startCallTimer() {
  state.callStartTime = Date.now();
  if (state.callTimerInterval) clearInterval(state.callTimerInterval);
  state.callTimerInterval = setInterval(() => {
    const elapsed = Math.floor((Date.now() - state.callStartTime) / 1000);
    const mins = String(Math.floor(elapsed / 60)).padStart(2, '0');
    const secs = String(elapsed % 60).padStart(2, '0');
    activeCallTimer.innerText = `${mins}:${secs}`;
  }, 1000);
}

function endActiveCall() {
  if (state.callTimerInterval) {
    clearInterval(state.callTimerInterval);
    state.callTimerInterval = null;
  }
  callOverlay.classList.add('hidden');
  incallKeypad.classList.add('hidden');
  state.currentCallId = null;
  state.isMuted = false;
  btnToggleMute.classList.remove('active');
}

// Call History Management
function saveHistoryItem(item) {
  state.callHistory.unshift(item);
  if (state.callHistory.length > 50) state.callHistory.pop();
  localStorage.setItem('rustphone_history', JSON.stringify(state.callHistory));
  renderHistory();
}

function renderHistory() {
  if (state.callHistory.length === 0) {
    historyList.innerHTML = `
      <div class="empty-state">
        <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="12" cy="12" r="10"></circle>
          <polyline points="12 6 12 12 16 14"></polyline>
        </svg>
        <p>No call history yet</p>
      </div>`;
    return;
  }

  historyList.innerHTML = state.callHistory.map(item => `
    <div class="history-item">
      <div class="history-meta">
        <span class="history-icon">${item.type === 'outgoing' ? '↗️' : '↙️'}</span>
        <div class="history-info">
          <div class="dest">${item.uri}</div>
          <div class="time">${new Date(item.time).toLocaleTimeString()}</div>
        </div>
      </div>
      <button class="icon-btn" onclick="startCall('${item.uri}')" title="Call back">📞</button>
    </div>
  `).join('');
}

btnClearHistory.addEventListener('click', () => {
  state.callHistory = [];
  localStorage.removeItem('rustphone_history');
  renderHistory();
});

// SIP Registration Form
const sipForm = document.getElementById('sip-form');
sipForm.addEventListener('submit', async (e) => {
  e.preventDefault();
  const config = {
    username: document.getElementById('acc-username').value.trim(),
    password: document.getElementById('acc-password').value,
    domain: document.getElementById('acc-domain').value.trim(),
    registrar: document.getElementById('acc-registrar').value.trim() || document.getElementById('acc-domain').value.trim(),
    expires: 3600,
  };

  localStorage.setItem('rustphone_account', JSON.stringify(config));
  statusPill.className = 'status-pill registering';
  statusText.innerText = 'Registering...';

  try {
    await invoke('register_account', { config });
  } catch (err) {
    statusPill.className = 'status-pill offline';
    statusText.innerText = 'Failed';
    alert('Registration error: ' + err);
  }
});

// Initialize SIP Stack and Tauri Event Listeners
async function initApp() {
  renderHistory();

  // Load saved account credentials if present
  const savedAcc = localStorage.getItem('rustphone_account');
  if (savedAcc) {
    try {
      const cfg = JSON.parse(savedAcc);
      document.getElementById('acc-username').value = cfg.username || '';
      document.getElementById('acc-password').value = cfg.password || '';
      document.getElementById('acc-domain').value = cfg.domain || '';
      document.getElementById('acc-registrar').value = cfg.registrar || '';
    } catch (_) {}
  }

  try {
    const epInfo = await invoke('init_sip_stack', { sipPort: 5060 });
    endpointAddrDisplay.innerText = `Bound to ${epInfo.sip_address}`;
    statusPill.className = 'status-pill offline';
    statusText.innerText = 'Ready';
  } catch (e) {
    endpointAddrDisplay.innerText = 'Error initializing SIP stack: ' + e;
  }

  // Populate audio hardware devices
  try {
    const [inputs, outputs] = await invoke('get_audio_devices');
    const inSelect = document.getElementById('audio-input');
    const outSelect = document.getElementById('audio-output');

    if (inputs.length > 0) inSelect.innerHTML = '';
    inputs.forEach(d => {
      const opt = document.createElement('option');
      opt.value = d.name;
      opt.innerText = (d.is_default ? '★ ' : '') + d.name;
      inSelect.appendChild(opt);
    });

    if (outputs.length > 0) outSelect.innerHTML = '';
    outputs.forEach(d => {
      const opt = document.createElement('option');
      opt.value = d.name;
      opt.innerText = (d.is_default ? '★ ' : '') + d.name;
      outSelect.appendChild(opt);
    });
  } catch (e) {
    console.warn('Audio devices fetch warning:', e);
  }

  // Listen to Tauri events from Rust backend
  if (isTauri && window.__TAURI__.event) {
    window.__TAURI__.event.listen('sip-event', (event) => {
      const data = event.payload;
      console.log('SIP Event received:', data);

      if (data.type === 'incoming_call') {
        showIncomingCall(data.call_id, data.from_uri);
      } else if (data.type === 'call_state') {
        if (data.state === 'Confirmed') {
          startCallTimer();
        } else if (data.state === 'Disconnected') {
          endActiveCall();
        } else if (data.state === 'Early') {
          activeCallTimer.innerText = 'Ringing...';
        }
      } else if (data.type === 'registration_state') {
        if (data.status_code === 200) {
          statusPill.className = 'status-pill registered';
          statusText.innerText = 'Registered';
        } else if (data.status_code >= 400) {
          statusPill.className = 'status-pill offline';
          statusText.innerText = `Error (${data.status_code})`;
        }
      }
    });
  }
}

window.addEventListener('DOMContentLoaded', initApp);
