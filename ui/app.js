// RustPhone Softphone Core Client
// High-Performance Web Audio DTMF + Fault-Tolerant Tauri IPC Bindings

// In-Memory Storage Fallback if localStorage is restricted
const memStorage = {};
function getStored(key, defaultVal) {
  try {
    const val = localStorage.getItem(key);
    return val !== null ? JSON.parse(val) : defaultVal;
  } catch (_) {
    return memStorage[key] !== undefined ? memStorage[key] : defaultVal;
  }
}

function setStored(key, val) {
  try {
    localStorage.setItem(key, JSON.stringify(val));
  } catch (_) {
    memStorage[key] = val;
  }
}

// DTMF ITU Frequency Table
const DTMF_FREQS = {
  '1': [697, 1209], '2': [697, 1336], '3': [697, 1477],
  '4': [770, 1209], '5': [770, 1336], '6': [770, 1477],
  '7': [852, 1209], '8': [852, 1336], '9': [852, 1477],
  '*': [941, 1209], '0': [941, 1336], '#': [941, 1477],
};

let audioCtx = null;
function playDtmfTone(digit, durationMs = 120) {
  try {
    const freqs = DTMF_FREQS[digit];
    if (!freqs) return;

    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) return;

    if (!audioCtx) {
      audioCtx = new AudioContextClass();
    }
    if (audioCtx.state === 'suspended') {
      audioCtx.resume();
    }

    const osc1 = audioCtx.createOscillator();
    const osc2 = audioCtx.createOscillator();
    const gain = audioCtx.createGain();

    osc1.frequency.value = freqs[0];
    osc2.frequency.value = freqs[1];

    gain.gain.setValueAtTime(0.08, audioCtx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, audioCtx.currentTime + (durationMs / 1000));

    osc1.connect(gain);
    osc2.connect(gain);
    gain.connect(audioCtx.destination);

    osc1.start();
    osc2.start();
    osc1.stop(audioCtx.currentTime + (durationMs / 1000));
    osc2.stop(audioCtx.currentTime + (durationMs / 1000));
  } catch (err) {
    console.warn('Audio tone synthesis bypassed:', err);
  }
}

// Universal Tauri IPC Invoker
async function callBackend(cmd, args = {}) {
  // Check Tauri 2.x global
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
    return await window.__TAURI__.core.invoke(cmd, args);
  }
  // Check Tauri 1.x / classic global
  if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
    return await window.__TAURI__.invoke(cmd, args);
  }

  console.info(`[Tauri Mock] ${cmd}`, args);
  if (cmd === 'init_sip_stack') {
    return { sip_address: '127.0.0.1:5060', is_initialized: true };
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
}

// Application State
const appState = {
  currentCallId: null,
  callStartTime: null,
  callTimerInterval: null,
  isMuted: false,
  isSpeakerMuted: false,
  isOnHold: false,
  callHistory: getStored('rustphone_history', []),
};

// Main DOM Setup
window.addEventListener('DOMContentLoaded', () => {
  // DOM Elements
  const dialInput = document.getElementById('dial-input');
  const btnBackspace = document.getElementById('btn-backspace');
  const btnCall = document.getElementById('btn-call');
  const statusPill = document.getElementById('status-pill');
  const statusText = document.getElementById('status-text');
  const endpointAddrDisplay = document.getElementById('endpoint-addr-display');
  const historyList = document.getElementById('history-list');
  const btnClearHistory = document.getElementById('btn-clear-history');

  const callOverlay = document.getElementById('active-call-overlay');
  const activeCallerName = document.getElementById('active-caller-name');
  const activeCallerUri = document.getElementById('active-caller-uri');
  const activeCallTimer = document.getElementById('active-call-timer');
  const btnHangup = document.getElementById('btn-hangup');
  const btnToggleMute = document.getElementById('btn-toggle-mute');
  const btnToggleSpeaker = document.getElementById('btn-toggle-speaker');
  const btnToggleHold = document.getElementById('btn-toggle-hold');
  const btnToggleKeypad = document.getElementById('btn-toggle-keypad');
  const incallKeypad = document.getElementById('incall-keypad');

  const incomingModal = document.getElementById('incoming-modal');
  const incomingCallerId = document.getElementById('incoming-caller-id');
  const incomingCallerUri = document.getElementById('incoming-caller-uri');
  const btnIncomingAccept = document.getElementById('btn-incoming-accept');
  const btnIncomingReject = document.getElementById('btn-incoming-reject');

  // Navigation Tabs Switching
  const navBtns = document.querySelectorAll('.nav-btn');
  const tabViews = document.querySelectorAll('.tab-view');

  navBtns.forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const tabName = btn.dataset.tab;
      navBtns.forEach(b => b.classList.remove('active'));
      tabViews.forEach(v => v.classList.remove('active'));

      btn.classList.add('active');
      const targetView = document.getElementById(`view-${tabName}`);
      if (targetView) {
        targetView.classList.add('active');
      }
    });
  });

  // Keypad Number Clicking
  document.querySelectorAll('.key-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      const key = btn.dataset.key;
      if (key && dialInput) {
        dialInput.value += key;
        dialInput.focus();
      }
      playDtmfTone(key);
    });
  });

  // Backspace Button
  if (btnBackspace && dialInput) {
    btnBackspace.addEventListener('click', (e) => {
      e.preventDefault();
      dialInput.value = dialInput.value.slice(0, -1);
      dialInput.focus();
    });
  }

  // Speed Dial Clicking in Contacts
  document.querySelectorAll('.contact-card').forEach(card => {
    const callBtn = card.querySelector('.quick-call-btn');
    const uri = card.dataset.uri;
    const trigger = (e) => {
      e.preventDefault();
      e.stopPropagation();
      if (dialInput) dialInput.value = uri;
      startCall(uri);
    };

    if (callBtn) callBtn.addEventListener('click', trigger);
    card.addEventListener('click', trigger);
  });

  // Call Initiation
  async function startCall(destination) {
    if (!destination) return;
    try {
      if (statusText) statusText.innerText = 'Calling...';
      const callId = await callBackend('make_call', { destination, accountId: 0 });
      appState.currentCallId = callId;
      showActiveCall(destination, destination, 'Calling...');
      saveHistoryItem({ uri: destination, type: 'outgoing', time: new Date().toISOString() });
    } catch (err) {
      alert('Call failed: ' + err);
      if (statusText) statusText.innerText = 'Call Failed';
    }
  }

  if (btnCall && dialInput) {
    btnCall.addEventListener('click', (e) => {
      e.preventDefault();
      const dest = dialInput.value.trim();
      if (dest) {
        startCall(dest);
      } else {
        dialInput.focus();
      }
    });
  }

  // Active Call Overlay Controls
  function showActiveCall(name, uri, status) {
    if (activeCallerName) activeCallerName.innerText = name;
    if (activeCallerUri) activeCallerUri.innerText = uri;
    if (activeCallTimer) activeCallTimer.innerText = status;
    if (callOverlay) callOverlay.classList.remove('hidden');
  }

  function startCallTimer() {
    appState.callStartTime = Date.now();
    if (activeCallTimer) activeCallTimer.innerText = '00:00';
    if (appState.callTimerInterval) clearInterval(appState.callTimerInterval);
    appState.callTimerInterval = setInterval(() => {
      const elapsed = Math.floor((Date.now() - appState.callStartTime) / 1000);
      const mins = String(Math.floor(elapsed / 60)).padStart(2, '0');
      const secs = String(elapsed % 60).padStart(2, '0');
      if (activeCallTimer) {
        if (appState.isOnHold) {
          activeCallTimer.innerText = `⏸️ HOLD (${mins}:${secs})`;
        } else {
          activeCallTimer.innerText = `${mins}:${secs}`;
        }
      }
    }, 1000);
  }

  function endActiveCall() {
    if (appState.callTimerInterval) {
      clearInterval(appState.callTimerInterval);
      appState.callTimerInterval = null;
    }
    if (callOverlay) callOverlay.classList.add('hidden');
    if (incallKeypad) incallKeypad.classList.add('hidden');
    appState.currentCallId = null;
    appState.isMuted = false;
    appState.isSpeakerMuted = false;
    appState.isOnHold = false;
    if (btnToggleMute) btnToggleMute.classList.remove('active');
    if (btnToggleSpeaker) btnToggleSpeaker.classList.remove('active');
    if (btnToggleHold) btnToggleHold.classList.remove('hold-active');
    if (activeCallTimer) activeCallTimer.classList.remove('on-hold');
    if (statusText) statusText.innerText = 'Ready';
  }

  if (btnHangup) {
    btnHangup.addEventListener('click', async (e) => {
      e.preventDefault();
      if (appState.currentCallId) {
        try {
          await callBackend('hangup_call', { callId: appState.currentCallId });
        } catch (err) {
          console.error('Hangup error:', err);
        }
      }
      endActiveCall();
    });
  }

  if (btnToggleMute) {
    btnToggleMute.addEventListener('click', async (e) => {
      e.preventDefault();
      if (!appState.currentCallId) return;
      appState.isMuted = !appState.isMuted;
      try {
        await callBackend('set_mute', { callId: appState.currentCallId, muteMic: appState.isMuted });
        btnToggleMute.classList.toggle('active', appState.isMuted);
      } catch (err) {
        console.error('Mute error:', err);
      }
    });
  }

  if (btnToggleSpeaker) {
    btnToggleSpeaker.addEventListener('click', async (e) => {
      e.preventDefault();
      if (!appState.currentCallId) return;
      appState.isSpeakerMuted = !appState.isSpeakerMuted;
      try {
        await callBackend('set_speaker_mute', { callId: appState.currentCallId, muteSpeaker: appState.isSpeakerMuted });
        btnToggleSpeaker.classList.toggle('active', appState.isSpeakerMuted);
      } catch (err) {
        console.error('Speaker mute error:', err);
      }
    });
  }

  if (btnToggleHold) {
    btnToggleHold.addEventListener('click', async (e) => {
      e.preventDefault();
      if (!appState.currentCallId) return;
      appState.isOnHold = !appState.isOnHold;
      try {
        await callBackend('set_hold', { callId: appState.currentCallId, hold: appState.isOnHold });
        btnToggleHold.classList.toggle('hold-active', appState.isOnHold);
        if (activeCallTimer) {
          activeCallTimer.classList.toggle('on-hold', appState.isOnHold);
        }
      } catch (err) {
        console.error('Hold error:', err);
      }
    });
  }

  if (btnToggleKeypad && incallKeypad) {
    btnToggleKeypad.addEventListener('click', (e) => {
      e.preventDefault();
      incallKeypad.classList.toggle('hidden');
    });
  }

  // In-call Mini DTMF Keypad
  document.querySelectorAll('.mini-key').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.preventDefault();
      const digit = btn.dataset.dtmf;
      playDtmfTone(digit);
      if (appState.currentCallId) {
        try {
          await callBackend('send_dtmf', { callId: appState.currentCallId, digit });
        } catch (err) {
          console.error('DTMF error:', err);
        }
      }
    });
  });

  // Incoming Call Handlers
  let incomingCallId = null;
  function showIncomingCall(callId, fromUri) {
    incomingCallId = callId;
    if (incomingCallerId) incomingCallerId.innerText = fromUri.split('@')[0].replace('sip:', '') || 'Incoming Call';
    if (incomingCallerUri) incomingCallerUri.innerText = fromUri;
    if (incomingModal) incomingModal.classList.remove('hidden');
  }

  if (btnIncomingAccept) {
    btnIncomingAccept.addEventListener('click', async (e) => {
      e.preventDefault();
      if (!incomingCallId) return;
      const id = incomingCallId;
      if (incomingModal) incomingModal.classList.add('hidden');
      try {
        await callBackend('answer_call', { callId: id, statusCode: 200 });
        appState.currentCallId = id;
        showActiveCall('Incoming Call', incomingCallerUri ? incomingCallerUri.innerText : id, 'Connected');
        startCallTimer();
      } catch (err) {
        alert('Failed to answer call: ' + err);
      }
    });
  }

  if (btnIncomingReject) {
    btnIncomingReject.addEventListener('click', async (e) => {
      e.preventDefault();
      if (!incomingCallId) return;
      if (incomingModal) incomingModal.classList.add('hidden');
      try {
        await callBackend('hangup_call', { callId: incomingCallId });
      } catch (err) {
        console.error(err);
      }
      incomingCallId = null;
    });
  }

  // Call History Management
  function saveHistoryItem(item) {
    appState.callHistory.unshift(item);
    if (appState.callHistory.length > 50) appState.callHistory.pop();
    setStored('rustphone_history', appState.callHistory);
    renderHistory();
  }

  function renderHistory() {
    if (!historyList) return;
    if (appState.callHistory.length === 0) {
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

    historyList.innerHTML = appState.callHistory.map(item => `
      <div class="history-item">
        <div class="history-meta">
          <span class="history-icon">${item.type === 'outgoing' ? '↗️' : '↙️'}</span>
          <div class="history-info">
            <div class="dest">${item.uri}</div>
            <div class="time">${new Date(item.time).toLocaleTimeString()}</div>
          </div>
        </div>
        <button class="icon-btn callback-btn" data-uri="${item.uri}" title="Call back">📞</button>
      </div>
    `).join('');

    // Attach callback click handlers
    historyList.querySelectorAll('.callback-btn').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        const uri = btn.dataset.uri;
        if (dialInput) dialInput.value = uri;
        startCall(uri);
      });
    });
  }

  if (btnClearHistory) {
    btnClearHistory.addEventListener('click', (e) => {
      e.preventDefault();
      appState.callHistory = [];
      setStored('rustphone_history', []);
      renderHistory();
    });
  }

  // SIP Account Settings Form
  const sipForm = document.getElementById('sip-form');
  if (sipForm) {
    sipForm.addEventListener('submit', async (e) => {
      e.preventDefault();
      const config = {
        username: document.getElementById('acc-username').value.trim(),
        password: document.getElementById('acc-password').value,
        domain: document.getElementById('acc-domain').value.trim(),
        registrar: document.getElementById('acc-registrar').value.trim() || document.getElementById('acc-domain').value.trim(),
        expires: 3600,
      };

      setStored('rustphone_account', config);
      if (statusPill) statusPill.className = 'status-pill registering';
      if (statusText) statusText.innerText = 'Registering...';

      try {
        await callBackend('register_account', { config });
      } catch (err) {
        if (statusPill) statusPill.className = 'status-pill offline';
        if (statusText) statusText.innerText = 'Failed';
        alert('Registration error: ' + err);
      }
    });
  }

  // Initialize Backend & UI
  async function init() {
    renderHistory();

    // Populate saved SIP account
    const saved = getStored('rustphone_account', null);
    if (saved) {
      if (document.getElementById('acc-username')) document.getElementById('acc-username').value = saved.username || '';
      if (document.getElementById('acc-password')) document.getElementById('acc-password').value = saved.password || '';
      if (document.getElementById('acc-domain')) document.getElementById('acc-domain').value = saved.domain || '';
      if (document.getElementById('acc-registrar')) document.getElementById('acc-registrar').value = saved.registrar || '';
    }

    // Initialize SIP stack
    try {
      const epInfo = await callBackend('init_sip_stack', { sipPort: 5060 });
      if (endpointAddrDisplay && epInfo) {
        endpointAddrDisplay.innerText = `Bound to ${epInfo.sip_address}`;
      }
      if (statusPill) statusPill.className = 'status-pill offline';
      if (statusText) statusText.innerText = 'Ready';
    } catch (err) {
      if (endpointAddrDisplay) endpointAddrDisplay.innerText = 'SIP stack init: ' + err;
    }

    // Fetch sound card devices
    try {
      const [inputs, outputs] = await callBackend('get_audio_devices');
      const inSelect = document.getElementById('audio-input');
      const outSelect = document.getElementById('audio-output');

      if (inSelect && inputs && inputs.length > 0) {
        inSelect.innerHTML = '';
        inputs.forEach(d => {
          const opt = document.createElement('option');
          opt.value = d.name;
          opt.innerText = (d.is_default ? '★ ' : '') + d.name;
          inSelect.appendChild(opt);
        });
      }

      if (outSelect && outputs && outputs.length > 0) {
        outSelect.innerHTML = '';
        outputs.forEach(d => {
          const opt = document.createElement('option');
          opt.value = d.name;
          opt.innerText = (d.is_default ? '★ ' : '') + d.name;
          outSelect.appendChild(opt);
        });
      }
    } catch (err) {
      console.warn('Audio devices fetch:', err);
    }

    // Subscribe to Tauri SIP events
    const eventSystem = (window.__TAURI__ && window.__TAURI__.event) || null;
    if (eventSystem && typeof eventSystem.listen === 'function') {
      eventSystem.listen('sip-event', (event) => {
        const data = event.payload;
        if (!data) return;

        if (data.type === 'incoming_call') {
          showIncomingCall(data.call_id, data.from_uri);
        } else if (data.type === 'call_state') {
          const st = (data.state || '').toLowerCase();
          if (st === 'confirmed' || st.includes('confirmed') || st === 'connecting') {
            startCallTimer();
          } else if (st === 'disconnected' || st.includes('disconnected')) {
            if (incomingCallId === data.call_id && incomingModal) {
              incomingModal.classList.add('hidden');
              incomingCallId = null;
            }
            endActiveCall();
          } else if (st === 'early' || st.includes('ringing') || st.includes('early')) {
            if (activeCallTimer) activeCallTimer.innerText = 'Ringing...';
          } else if (st === 'calling') {
            if (activeCallTimer) activeCallTimer.innerText = 'Calling...';
          }
        } else if (data.type === 'registration_state') {
          if (data.status_code === 200) {
            if (statusPill) statusPill.className = 'status-pill registered';
            if (statusText) statusText.innerText = 'Registered';
          } else if (data.status_code >= 400) {
            if (statusPill) statusPill.className = 'status-pill offline';
            if (statusText) statusText.innerText = `Error (${data.status_code})`;
          }
        }
      });
    }
  }

  init();
});
