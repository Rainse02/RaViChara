// RaViChara client application

document.addEventListener('DOMContentLoaded', () => {
  const threeDotsBtn = document.getElementById('threeDotsBtn');
  const glassSettingsOverlay = document.getElementById('glassSettingsOverlay');
  const closeSettingsPanel = document.getElementById('closeSettingsPanel');

  const viewportMainBody = document.getElementById('viewportMainBody');
  const renderingWindowPane = document.getElementById('renderingWindowPane');
  const paneResizerHandle = document.getElementById('paneResizerHandle');
  const compactAvatarBadge = document.getElementById('compactAvatarBadge');
  const desktopBlurredBg = document.getElementById('desktopBlurredBg');
  const initialChatTimestamp = document.getElementById('initialChatTimestamp');
  const abstractPillText = document.getElementById('abstractPillText');
  const desktopHeader = document.querySelector('.outer-hui-header-bar');
  const minimizeWindowBtn = document.querySelector('.minimize-btn');
  const maximizeWindowBtn = document.querySelector('.maximize-btn');
  const closeWindowBtn = document.querySelector('.close-app-btn');

  const chatInput = document.getElementById('chatInput');
  const sendBtn = document.getElementById('sendBtn');
  const chatHistory = document.getElementById('chatHistory');
  const micBtn = document.getElementById('micBtn');

  // Tab Buttons & Content Panes
  const tabBtns = document.querySelectorAll('.settings-sidebar-tabs .tab-btn');
  const tabPanes = document.querySelectorAll('.settings-tab-content .tab-pane');

  // LLM & Interval Controls
  const saveLlmBtn = document.getElementById('saveLlmBtn');
  const timestampIntervalInput = document.getElementById('timestampIntervalInput');
  const llmProviderSelectWrapper = document.getElementById('llmProviderSelectWrapper');
  const llmApiKey = document.getElementById('llmApiKey');
  const llmBaseUrl = document.getElementById('llmBaseUrl');
  const llmModel = document.getElementById('llmModel');
  const llmMaxTokens = document.getElementById('llmMaxTokens');
  const llmExampleLimit = document.getElementById('llmExampleLimit');
  const llmThinkingSelectWrapper = document.getElementById('llmThinkingSelectWrapper');

  // Voice controls
  const ttsProviderSelectWrapper = document.getElementById('ttsProviderSelectWrapper');
  const ttsFormatSelectWrapper = document.getElementById('ttsFormatSelectWrapper');
  const ttsBaseUrl = document.getElementById('ttsBaseUrl');
  const ttsModel = document.getElementById('ttsModel');
  const ttsVoice = document.getElementById('ttsVoice');
  const ttsApiKey = document.getElementById('ttsApiKey');
  const ttsSpeed = document.getElementById('ttsSpeed');
  const ttsStyle = document.getElementById('ttsStyle');
  const ttsAutoPlay = document.getElementById('ttsAutoPlay');
  const saveTtsBtn = document.getElementById('saveTtsBtn');
  const testTtsBtn = document.getElementById('testTtsBtn');
  const ttsStatusText = document.getElementById('ttsStatusText');

  // Blender and render-stage controls
  const blenderProfileSelectWrapper = document.getElementById('blenderProfileSelectWrapper');
  const renderSourceSelectWrapper = document.getElementById('renderSourceSelectWrapper');
  const blenderStreamSizeModeWrapper = document.getElementById('blenderStreamSizeModeWrapper');
  const blenderBridgeStatus = document.getElementById('blenderBridgeStatus');
  const blenderPreviewStatus = document.getElementById('blenderPreviewStatus');
  const activeRenderSource = document.getElementById('activeRenderSource');
  const blenderCameraStatus = document.getElementById('blenderCameraStatus');
  const blenderStreamStatus = document.getElementById('blenderStreamStatus');
  const blenderStreamWidth = document.getElementById('blenderStreamWidth');
  const blenderStreamHeight = document.getElementById('blenderStreamHeight');
  const blenderStreamFps = document.getElementById('blenderStreamFps');
  const blenderPlaybackStart = document.getElementById('blenderPlaybackStart');
  const blenderActionStart = document.getElementById('blenderActionStart');
  const blenderActionEnd = document.getElementById('blenderActionEnd');
  const blenderPlaybackEnd = document.getElementById('blenderPlaybackEnd');
  const blenderTransitionFrames = document.getElementById('blenderTransitionFrames');
  const blenderControlAnalysisEnabled = document.getElementById('blenderControlAnalysisEnabled');
  const blenderControlAnalysisModel = document.getElementById('blenderControlAnalysisModel');
  const blenderSceneProposalsEnabled = document.getElementById('blenderSceneProposalsEnabled');
  const saveBlenderSettingsBtn = document.getElementById('saveBlenderSettingsBtn');
  const testBlenderBtn = document.getElementById('testBlenderBtn');
  const refreshBlenderFrameBtn = document.getElementById('refreshBlenderFrameBtn');
  const stopBlenderAnimationBtn = document.getElementById('stopBlenderAnimationBtn');
  const test2dMotionBtn = document.getElementById('test2dMotionBtn');
  const testVideoSourceBtn = document.getElementById('testVideoSourceBtn');
  const blenderDiagnosticOutput = document.getElementById('blenderDiagnosticOutput');
  const blenderFrameImg = document.getElementById('blenderFrameImg');
  const externalVideoFrame = document.getElementById('externalVideoFrame');
  const videoEnabled = document.getElementById('videoEnabled');
  const videoSourceTypeWrapper = document.getElementById('videoSourceTypeWrapper');
  const videoSourceUrl = document.getElementById('videoSourceUrl');
  const videoAutoplay = document.getElementById('videoAutoplay');
  const videoLoop = document.getElementById('videoLoop');
  const videoMuted = document.getElementById('videoMuted');
  const renderStageStatus = document.getElementById('renderStageStatus');
  const renderStageStatusText = document.getElementById('renderStageStatusText');
  const characterAvatar = document.getElementById('characterAvatar');
  const characterStageFx = document.getElementById('characterStageFx');

  // Memory Panel
  const activeCharMemoryName = document.getElementById('activeCharMemoryName');
  const memoryDetailContainer = document.getElementById('memoryDetailContainer');

  // Persona Grid
  const personaSettingsGrid = document.getElementById('personaSettingsGrid');

  // Upload & Picker Elements
  const bgFileInput = document.getElementById('bgFileInput');
  const resetBgBtn = document.getElementById('resetBgBtn');
  const mainBackdropImageLayer = document.getElementById('mainBackdropImageLayer');

  const avatarFileInput = document.getElementById('avatarFileInput');
  const resetAvatarBtn = document.getElementById('resetAvatarBtn');
  const avatarImg = document.getElementById('avatarImg');
  const avatarSvg = document.getElementById('avatarSvg');
  const badgeAvatarImg = document.getElementById('badgeAvatarImg');
  const badgeFallbackIcon = document.getElementById('badgeFallbackIcon');

  // Color Pickers
  const mainBackdropColorPicker = document.getElementById('mainBackdropColorPicker');
  const userBubbleColorPicker = document.getElementById('userBubbleColorPicker');
  const accentColorPicker = document.getElementById('accentColorPicker');
  const characterBubbleColorPicker = document.getElementById('characterBubbleColorPicker');
  const renderBackdropColorPicker = document.getElementById('renderBackdropColorPicker');

  const mainColorHexText = document.getElementById('mainColorHexText');
  const userColorHexText = document.getElementById('userColorHexText');
  const accentColorHexText = document.getElementById('accentColorHexText');
  const charColorHexText = document.getElementById('charColorHexText');
  const renderColorHexText = document.getElementById('renderColorHexText');

  let activeCharacterName = "Lily";
  let activePersonaAvatarUrl = '/api/persona/avatar';
  let hasCustomAvatar = false;
  let lastMessageTime = Date.now();
  let chatRequestPending = false;
  let llmProviderPresets = new Map();
  let ttsProviderPresets = new Map();
  let ttsAutoPlayEnabled = false;
  let activeAudio = null;
  let blenderStreamSocket = null;
  let blenderStreamReconnectTimer = null;
  let blenderStreamGeneration = 0;
  let blenderStreamReconnectAttempts = 0;
  let interactionStreamSocket = null;
  let interactionStreamConnected = false;
  let interactionReconnectTimer = null;
  let emoteResetTimer = null;
  let motionResetTimer = null;
  let activeGreeting = '';
  let loadedChatMessages = [];
  let chatHistoryHasMore = false;
  let chatHistoryBeforeId = null;
  let renderSource = localStorage.getItem('render_source') || '2d';
  let videoSettings = {
    enabled: false,
    sourceType: 'video',
    sourceUrl: '',
    autoplay: true,
    loop: true,
    muted: true
  };
  let hasCustomBackground = false;
  let lastRenderFrameSize = { width: 512, height: 512 };
  let renderPaneManuallySized = false;
  let configuredTimestampIntervalMin = parseInt(localStorage.getItem('timestamp_interval_min') || '10', 10);

  function postDesktopCommand(command) {
    if (
      (window.__RAVICHARA_DESKTOP__ || window.__EVERCHARA_DESKTOP__)
      && window.ipc
      && typeof window.ipc.postMessage === 'function'
    ) {
      window.ipc.postMessage(command);
      return true;
    }
    return false;
  }

  minimizeWindowBtn?.addEventListener('click', event => {
    event.stopPropagation();
    postDesktopCommand('minimize');
  });
  maximizeWindowBtn?.addEventListener('click', event => {
    event.stopPropagation();
    postDesktopCommand('maximize');
  });
  closeWindowBtn?.addEventListener('click', event => {
    event.stopPropagation();
    if (!postDesktopCommand('close')) window.close();
  });
  desktopHeader?.addEventListener('pointerdown', event => {
    if (
      event.button !== 0
      || event.target.closest('button, input, select, textarea, a')
    ) {
      return;
    }
    postDesktopCommand('drag');
  });
  desktopHeader?.addEventListener('dblclick', event => {
    if (event.target.closest('button, input, select, textarea, a')) return;
    postDesktopCommand('maximize');
  });

  if (timestampIntervalInput) {
    timestampIntervalInput.value = configuredTimestampIntervalMin;
    timestampIntervalInput.addEventListener('change', (e) => {
      let val = parseInt(e.target.value, 10);
      if (isNaN(val) || val < 1) val = 1;
      configuredTimestampIntervalMin = val;
      localStorage.setItem('timestamp_interval_min', val);
      showTemporaryNoticePill(`沉浸时间感知间隔设为 ${val} 分钟`);
    });
  }

  // Preset Theme Color RGB & Hex Maps
  const presetThemeMap = {
    "sunset": { rgb: "217, 119, 6", hex: "#3B2010" },
    "midnight": { rgb: "59, 130, 246", hex: "#1E1B4B" },
    "cyber": { rgb: "16, 185, 129", hex: "#064E3B" },
    "warm-beige": { rgb: "159, 243, 254", hex: "#FEF3C7" }
  };

  // Set initial timestamp format
  function updateInitialTimestamp() {
    const now = new Date();
    const year = now.getFullYear();
    const month = String(now.getMonth() + 1).padStart(2, '0');
    const day = String(now.getDate()).padStart(2, '0');
    const hours = String(now.getHours()).padStart(2, '0');
    const minutes = String(now.getMinutes()).padStart(2, '0');

    if (initialChatTimestamp) {
      initialChatTimestamp.textContent = `${year}-${month}-${day} ${hours}:${minutes}`;
    }
  }
  updateInitialTimestamp();

  // Restore saved themes & colors
  const savedMainSceneColor = localStorage.getItem('main_scene_backdrop_bg');
  const savedTheme = localStorage.getItem('ui_bg_theme') || 'warm-beige';

  if (savedMainSceneColor) {
    setMainSceneBackdropColor(savedMainSceneColor);
  } else {
    setThemePreset(savedTheme);
  }

  const savedUserColor = localStorage.getItem('user_bubble_hex') || '#B6EBEC';
  setUserBubbleColor(savedUserColor);

  const savedAccentColor = localStorage.getItem('ui_accent_color') || '#9FF3FE';
  setAccentColor(savedAccentColor);

  const savedCharBubbleColor = localStorage.getItem('char_bubble_bg') || '#FFFFFF';
  setCharBubbleColor(savedCharBubbleColor);

  const storedRenderBackdropColor = localStorage.getItem('render_backdrop_bg');
  const savedRenderBackdropColor = /^#[0-9a-f]{6}$/i.test(storedRenderBackdropColor || '')
    ? storedRenderBackdropColor
    : '#BDBBF7';
  setRenderBackdropColor(savedRenderBackdropColor);

  updateAbstractHeaderPill();
  initCustomGlassSelects();
  attachReplayControl(document.getElementById('greetingBubble')?.closest('.chat-bubble'));
  connectInteractionStream();
  applyRenderSource(renderSource);
  bootstrapBackendState();

  // Automatic Contrast Calculator
  function getContrastTextColor(hex) {
    if (!hex || typeof hex !== 'string') return '#FFFFFF';
    let cleanHex = hex.replace("#", "");
    if (cleanHex.length === 3) {
      cleanHex = cleanHex.split('').map(c => c + c).join('');
    }
    const r = parseInt(cleanHex.substring(0, 2), 16) || 0;
    const g = parseInt(cleanHex.substring(2, 4), 16) || 0;
    const b = parseInt(cleanHex.substring(4, 6), 16) || 0;

    const yiq = ((r * 299) + (g * 587) + (b * 114)) / 1000;
    return (yiq >= 150) ? '#0F172A' : '#FFFFFF';
  }

  // Update Settings Panel Controls Contrast & Colors
  function updateSettingsPanelContrast(hexColor) {
    const textColor = getContrastTextColor(hexColor);
    const isLight = (textColor === '#0F172A');

    document.documentElement.style.setProperty('--glass-panel-bg', isLight ? 'rgba(255, 255, 255, 0.92)' : 'rgba(15, 23, 42, 0.85)');
    document.documentElement.style.setProperty('--glass-sidebar-bg', isLight ? 'rgba(241, 245, 249, 0.85)' : 'rgba(30, 41, 59, 0.65)');
    document.documentElement.style.setProperty('--glass-panel-text', textColor);

    document.documentElement.style.setProperty('--glass-input-bg', isLight ? 'rgba(241, 245, 249, 0.95)' : 'rgba(30, 41, 59, 0.65)');
    document.documentElement.style.setProperty('--glass-input-border', isLight ? 'rgba(0, 0, 0, 0.15)' : 'rgba(255, 255, 255, 0.2)');
    document.documentElement.style.setProperty('--glass-input-text', textColor);
    document.documentElement.style.setProperty('--glass-placeholder-color', isLight ? '#64748b' : '#94a3b8');

    // Timestamp Contrast Update
    document.documentElement.style.setProperty('--timestamp-bg', isLight ? 'rgba(15, 23, 42, 0.15)' : 'rgba(255, 255, 255, 0.18)');
    document.documentElement.style.setProperty('--timestamp-text-color', textColor);
    document.documentElement.style.setProperty('--timestamp-border', isLight ? 'rgba(15, 23, 42, 0.25)' : 'rgba(255, 255, 255, 0.25)');
    updateWindowControlContrast(hexColor);
  }

  function updateWindowControlContrast(hexColor) {
    const isLight = getContrastTextColor(hexColor) === '#0F172A';
    document.documentElement.style.setProperty(
      '--window-control-color',
      isLight ? '#0F172A' : '#FFFFFF'
    );
    document.documentElement.style.setProperty(
      '--window-control-bg',
      isLight ? 'rgba(255, 255, 255, 0.44)' : 'rgba(0, 0, 0, 0.22)'
    );
    document.documentElement.style.setProperty(
      '--window-control-border',
      isLight ? 'rgba(15, 23, 42, 0.28)' : 'rgba(255, 255, 255, 0.32)'
    );
    document.documentElement.style.setProperty(
      '--window-control-hover-bg',
      isLight ? 'rgba(255, 255, 255, 0.72)' : 'rgba(0, 0, 0, 0.38)'
    );
  }

  // Custom Glass Select Components Logic
  function initCustomGlassSelects() {
    document.querySelectorAll('.custom-glass-select-wrapper').forEach(wrapper => {
      const trigger = wrapper.querySelector('.custom-select-trigger');
      const menu = wrapper.querySelector('.custom-select-menu');
      trigger.addEventListener('click', (e) => {
        e.stopPropagation();
        document.querySelectorAll('.custom-select-menu').forEach(m => {
          if (m !== menu) m.classList.remove('open');
        });
        menu.classList.toggle('open');
      });

      bindCustomSelectOptions(wrapper);
    });

    document.addEventListener('click', () => {
      document.querySelectorAll('.custom-select-menu').forEach(m => m.classList.remove('open'));
    });
  }

  function bindCustomSelectOptions(wrapper) {
      const menu = wrapper.querySelector('.custom-select-menu');
      const selectedText = wrapper.querySelector('.selected-text');
      const options = menu.querySelectorAll('.custom-option');
      options.forEach(opt => {
        opt.addEventListener('click', () => {
          options.forEach(o => o.classList.remove('selected'));
          opt.classList.add('selected');
          selectedText.textContent = opt.textContent;
          wrapper.dataset.value = opt.dataset.value || '';
          menu.classList.remove('open');
          wrapper.dispatchEvent(new CustomEvent('custom-select-change', {
            detail: { value: wrapper.dataset.value }
          }));
        });
      });

      const selected = menu.querySelector('.custom-option.selected');
      wrapper.dataset.value = selected ? (selected.dataset.value || '') : '';
  }

  function renderProviderOptions(providers, selectedProvider) {
    const menu = llmProviderSelectWrapper.querySelector('.custom-select-menu');
    menu.replaceChildren();
    llmProviderPresets = new Map();
    providers.forEach(provider => {
      llmProviderPresets.set(provider.id, provider);
      if (provider.id === 'mock') return;
      const option = document.createElement('div');
      option.className = 'custom-option';
      option.dataset.value = provider.id;
      option.textContent = provider.label;
      option.classList.toggle('selected', provider.id === selectedProvider);
      menu.appendChild(option);
    });
    bindCustomSelectOptions(llmProviderSelectWrapper);
  }

  function renderTtsProviderOptions(providers, selectedProvider) {
    const menu = ttsProviderSelectWrapper.querySelector('.custom-select-menu');
    menu.replaceChildren();
    ttsProviderPresets = new Map();
    providers.forEach(provider => {
      ttsProviderPresets.set(provider.id, provider);
      const option = document.createElement('div');
      option.className = 'custom-option';
      option.dataset.value = provider.id;
      option.textContent = provider.label;
      option.classList.toggle('selected', provider.id === selectedProvider);
      menu.appendChild(option);
    });
    bindCustomSelectOptions(ttsProviderSelectWrapper);
  }

  function setCustomSelectValue(wrapper, value) {
    if (!wrapper) return;
    const options = wrapper.querySelectorAll('.custom-option');
    const selectedText = wrapper.querySelector('.selected-text');
    let matched = null;
    options.forEach(option => {
      const isSelected = option.dataset.value === value;
      option.classList.toggle('selected', isSelected);
      if (isSelected) matched = option;
    });
    if (matched) {
      wrapper.dataset.value = value;
      selectedText.textContent = matched.textContent;
    }
  }

  function stripControlTags(text) {
    return String(text || '')
      .replace(/\[(?:emote|motion):[A-Za-z0-9_-]{1,64}\]/g, '')
      .replace(/\[(?:(?:ravi|ever)chara_set_expression|(?:ravi|ever)chara_play_motion)[^\]]*\]/gi, '')
      .replace(/^\s*(?:(?:ravi|ever)chara_set_expression|(?:ravi|ever)chara_play_motion)\s*[:=(].*$/gmi, '')
      .trim();
  }

  async function apiErrorMessage(response, fallback) {
    try {
      const payload = await response.json();
      return payload?.error?.message || payload?.detail || fallback;
    } catch (_) {
      return fallback;
    }
  }

  async function bootstrapBackendState() {
    try {
      const [
        statusResponse,
        personaResponse,
        settingsResponse,
        providersResponse,
        ttsProvidersResponse
      ] = await Promise.all([
        fetch('/api/status'),
        fetch('/api/persona'),
        fetch('/api/settings'),
        fetch('/api/llm/providers'),
        fetch('/api/tts/providers')
      ]);
      if (
        !statusResponse.ok
        || !personaResponse.ok
        || !settingsResponse.ok
        || !providersResponse.ok
        || !ttsProvidersResponse.ok
      ) {
        throw new Error('后端初始化接口返回异常');
      }

      const status = await statusResponse.json();
      const persona = await personaResponse.json();
      const settings = await settingsResponse.json();
      const providerData = await providersResponse.json();
      const ttsProviderData = await ttsProvidersResponse.json();
      activeCharacterName = status.active_character || persona.name || activeCharacterName;
      activePersonaAvatarUrl = persona.avatar
        ? `/api/persona/avatar?v=${Date.now()}`
        : '';
      updateAbstractHeaderPill();
      activeCharMemoryName.textContent = activeCharacterName;

      const greetingBubble = document.getElementById('greetingBubble');
      if (greetingBubble) {
        const greeting = typeof persona.greeting === 'string'
          ? persona.greeting
          : (persona.greeting?.zh || persona.greeting?.en || '');
        activeGreeting = stripControlTags(greeting)
          || `你好，我是 ${activeCharacterName}。`;
        greetingBubble.textContent = activeGreeting;
      }

      renderProviderOptions(providerData.providers || [], settings.llm?.provider);
      setCustomSelectValue(llmProviderSelectWrapper, settings.llm?.provider);
      llmBaseUrl.value = settings.llm?.base_url || settings.llm?.resolved_base_url || '';
      llmModel.value = settings.llm?.model || '';
      llmMaxTokens.value = settings.llm?.max_tokens || 2048;
      llmExampleLimit.value = settings.llm?.example_dialogue_limit ?? 1;
      setCustomSelectValue(
        llmThinkingSelectWrapper,
        settings.llm?.thinking_mode || 'disabled'
      );
      updateApiKeyPlaceholder(
        llmApiKey,
        Boolean(settings.llm?.api_key_configured),
        llmProviderPresets.get(settings.llm?.provider)
      );

      const ttsSettings = settings.voice?.tts || {};
      renderTtsProviderOptions(ttsProviderData.providers || [], ttsSettings.provider || 'none');
      setCustomSelectValue(ttsProviderSelectWrapper, ttsSettings.provider || 'none');
      ttsBaseUrl.value = ttsSettings.base_url || '';
      ttsModel.value = ttsSettings.model || '';
      ttsVoice.value = ttsSettings.voice || '';
      ttsSpeed.value = ttsSettings.speed || 1;
      ttsStyle.value = ttsSettings.style || '';
      ttsAutoPlay.checked = Boolean(ttsSettings.auto_play);
      ttsAutoPlayEnabled = Boolean(ttsSettings.auto_play);
      setCustomSelectValue(ttsFormatSelectWrapper, ttsSettings.response_format || 'mp3');
      updateApiKeyPlaceholder(
        ttsApiKey,
        Boolean(ttsSettings.api_key_configured),
        ttsProviderPresets.get(ttsSettings.provider)
      );
      ttsStatusText.textContent = settings.voice?.status?.detail || 'TTS 状态未知。';

      setCustomSelectValue(
        blenderProfileSelectWrapper,
        settings.blender?.render_mode || 'off'
      );
      setCustomSelectValue(renderSourceSelectWrapper, renderSource);
      setCustomSelectValue(
        blenderStreamSizeModeWrapper,
        settings.blender?.stream_size_mode || 'camera'
      );
      blenderStreamWidth.value = settings.blender?.stream_size?.[0] || 512;
      blenderStreamHeight.value = settings.blender?.stream_size?.[1] || 512;
      blenderStreamFps.value = settings.blender?.stream_fps || 12;
      blenderPlaybackStart.value = settings.blender?.playback_range?.[0] ?? 1;
      blenderPlaybackEnd.value = settings.blender?.playback_range?.[1] ?? 49;
      blenderActionStart.value = settings.blender?.action_range?.[0] ?? 9;
      blenderActionEnd.value = settings.blender?.action_range?.[1] ?? 41;
      blenderTransitionFrames.value = settings.blender?.transition_frames ?? 8;
      blenderControlAnalysisEnabled.checked =
        settings.blender?.control_analysis_enabled !== false;
      blenderControlAnalysisModel.value =
        settings.blender?.control_analysis_model || '';
      blenderSceneProposalsEnabled.checked =
        settings.blender?.scene_proposals_enabled !== false;
      const configuredVideo = settings.video || {};
      videoSettings = {
        enabled: Boolean(configuredVideo.enabled),
        sourceType: configuredVideo.source_type || 'video',
        sourceUrl: configuredVideo.source_url || '',
        autoplay: configuredVideo.autoplay !== false,
        loop: configuredVideo.loop_playback !== false,
        muted: configuredVideo.muted !== false
      };
      videoEnabled.checked = videoSettings.enabled;
      setCustomSelectValue(videoSourceTypeWrapper, videoSettings.sourceType);
      videoSourceUrl.value = videoSettings.sourceUrl;
      videoAutoplay.checked = videoSettings.autoplay;
      videoLoop.checked = videoSettings.loop;
      videoMuted.checked = videoSettings.muted;
      if (renderSource === 'video') applyRenderSource('video');
      if (settings.llm?.mcp_enabled && !settings.llm?.mcp_model_access) {
        blenderDiagnosticOutput.textContent =
          'MCP 服务已在线；LM Studio 尚未授权 API 调用 mcp.json 插件，当前使用后端动作分析回退。';
      }
      await restoreUiAssets(settings.ui_assets || {});
      await loadChatHistory(true);
      await loadBlenderProposals();
      await refreshBlenderStatus();

      if (status.llm?.mode !== 'live') {
        showTemporaryNoticePill(`LLM 当前模式：${status.llm?.mode || 'unknown'}`);
      }
    } catch (error) {
      showTemporaryNoticePill(`后端未就绪：${error.message}`);
    }
  }

  function updateApiKeyPlaceholder(input, configured, preset) {
    if (!input) return;
    if (configured) {
      input.placeholder = '已保存在本地配置；留空保持现有密钥';
      input.dataset.keyConfigured = 'true';
      return;
    }
    input.dataset.keyConfigured = 'false';
    input.placeholder = preset?.api_key_env
      ? `可输入密钥，或使用环境变量 ${preset.api_key_env}`
      : (preset?.backend ? '可选 API Key' : '本地服务通常无需 API Key');
  }

  function applyRenderSource(source) {
    renderSource = ['2d', 'blender', 'video'].includes(source) ? source : '2d';
    localStorage.setItem('render_source', renderSource);
    setCustomSelectValue(renderSourceSelectWrapper, renderSource);
    activeRenderSource.textContent = {
      blender: 'Blender',
      video: 'Video',
      '2d': '2D'
    }[renderSource];
    if (renderSource === '2d') {
      stopBlenderStream();
      stopExternalVideo();
      blenderFrameImg.hidden = true;
      blenderFrameImg.classList.remove('is-visible');
      characterAvatar.style.opacity = '1';
      renderStageStatus.classList.remove('is-error', 'is-live');
      renderStageStatusText.textContent = '2D 轻量模式';
    } else if (renderSource === 'blender') {
      stopExternalVideo();
      blenderFrameImg.removeAttribute('src');
      blenderFrameImg.hidden = true;
      blenderFrameImg.classList.remove('is-visible');
      characterAvatar.style.opacity = '1';
      renderStageStatus.classList.remove('is-error', 'is-live');
      renderStageStatusText.textContent = '正在连接 Blender 实时画面';
      startBlenderStream();
    } else {
      stopBlenderStream();
      startExternalVideo();
    }
  }

  function stopExternalVideo() {
    externalVideoFrame.pause();
    externalVideoFrame.removeAttribute('src');
    externalVideoFrame.load();
    externalVideoFrame.hidden = true;
    externalVideoFrame.classList.remove('is-visible');
    if (renderSource !== 'blender') {
      blenderFrameImg.removeAttribute('src');
      blenderFrameImg.hidden = true;
      blenderFrameImg.classList.remove('is-visible');
    }
  }

  function externalVideoReady(width, height, mode) {
    if (renderSource !== 'video') return;
    lastRenderFrameSize = {
      width: Number(width) || lastRenderFrameSize.width,
      height: Number(height) || lastRenderFrameSize.height
    };
    if (!renderPaneManuallySized) syncRenderPaneToFrame();
    characterAvatar.style.opacity = '0';
    renderStageStatus.classList.remove('is-error');
    renderStageStatus.classList.add('is-live');
    renderStageStatusText.textContent =
      `外部视频 · ${lastRenderFrameSize.width}×${lastRenderFrameSize.height} · ${mode}`;
  }

  function startExternalVideo() {
    stopExternalVideo();
    renderStageStatus.classList.remove('is-live', 'is-error');
    const url = String(videoSettings.sourceUrl || '').trim();
    if (!videoSettings.enabled || !url) {
      characterAvatar.style.opacity = '1';
      renderStageStatus.classList.add('is-error');
      renderStageStatusText.textContent = '外部视频未启用或尚未配置 URL';
      return;
    }
    characterAvatar.style.opacity = '0';
    renderStageStatusText.textContent = '正在连接外部视频源';
    if (videoSettings.sourceType === 'mjpeg') {
      blenderFrameImg.onload = () => {
        if (renderSource !== 'video' || videoSettings.sourceType !== 'mjpeg') return;
        externalVideoReady(
          blenderFrameImg.naturalWidth,
          blenderFrameImg.naturalHeight,
          'MJPEG'
        );
      };
      blenderFrameImg.onerror = () => {
        if (renderSource !== 'video') return;
        renderStageStatus.classList.add('is-error');
        renderStageStatusText.textContent = 'MJPEG / 图像流加载失败';
      };
      blenderFrameImg.src = url;
      blenderFrameImg.hidden = false;
      blenderFrameImg.classList.add('is-visible');
      return;
    }
    externalVideoFrame.loop = videoSettings.loop;
    externalVideoFrame.muted = videoSettings.muted;
    externalVideoFrame.autoplay = videoSettings.autoplay;
    externalVideoFrame.src = url;
    externalVideoFrame.hidden = false;
    externalVideoFrame.classList.add('is-visible');
    externalVideoFrame.load();
    if (videoSettings.autoplay) {
      externalVideoFrame.play().catch(() => {
        if (renderSource !== 'video') return;
        renderStageStatusText.textContent = '视频已载入；浏览器要求手动开始播放';
      });
    }
  }

  externalVideoFrame.addEventListener('loadedmetadata', () => {
    externalVideoReady(
      externalVideoFrame.videoWidth,
      externalVideoFrame.videoHeight,
      'video'
    );
  });
  externalVideoFrame.addEventListener('error', () => {
    if (renderSource !== 'video' || videoSettings.sourceType !== 'video') return;
    renderStageStatus.classList.remove('is-live');
    renderStageStatus.classList.add('is-error');
    renderStageStatusText.textContent = '外部视频源加载失败';
  });

  function websocketUrl(path) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}${path}`;
  }

  function stopBlenderStream(resetBackoff = true) {
    blenderStreamGeneration += 1;
    if (resetBackoff) blenderStreamReconnectAttempts = 0;
    if (blenderStreamReconnectTimer) {
      window.clearTimeout(blenderStreamReconnectTimer);
      blenderStreamReconnectTimer = null;
    }
    if (blenderStreamSocket) {
      blenderStreamSocket.onclose = null;
      blenderStreamSocket.close();
      blenderStreamSocket = null;
    }
  }

  function startBlenderStream(forceReconnect = false) {
    if (renderSource !== 'blender') return;
    if (
      !forceReconnect
      && blenderStreamSocket
      && (blenderStreamSocket.readyState === WebSocket.OPEN
        || blenderStreamSocket.readyState === WebSocket.CONNECTING)
    ) {
      return;
    }
    if (forceReconnect) blenderStreamReconnectAttempts = 0;
    stopBlenderStream(false);
    const generation = blenderStreamGeneration;
    const socket = new WebSocket(websocketUrl('/api/blender/stream'));
    blenderStreamSocket = socket;
    renderStageStatus.classList.remove('is-error', 'is-live');
    renderStageStatusText.textContent = '正在连接 Blender 实时画面';

    socket.onopen = () => {
      if (generation !== blenderStreamGeneration) return;
      renderStageStatusText.textContent = 'Blender 实时流已连接';
    };
    socket.onmessage = (event) => {
      if (generation !== blenderStreamGeneration || renderSource !== 'blender') return;
      let payload;
      try {
        payload = JSON.parse(event.data);
      } catch (_) {
        return;
      }
      if (payload.type === 'frame' && payload.data_url) {
        blenderStreamReconnectAttempts = 0;
        lastRenderFrameSize = {
          width: Number(payload.width) || lastRenderFrameSize.width,
          height: Number(payload.height) || lastRenderFrameSize.height
        };
        if (!renderPaneManuallySized) syncRenderPaneToFrame();
        blenderFrameImg.src = payload.data_url;
        externalVideoFrame.hidden = true;
        externalVideoFrame.classList.remove('is-visible');
        blenderFrameImg.hidden = false;
        blenderFrameImg.classList.add('is-visible');
        characterAvatar.style.opacity = '0';
        renderStageStatus.classList.remove('is-error');
        renderStageStatus.classList.add('is-live');
        renderStageStatusText.textContent =
          `Blender 实时 · ${payload.width}×${payload.height} · ${payload.capture_mode || 'camera'}`;
        blenderStreamStatus.textContent =
          `${payload.width}×${payload.height} · ${blenderStreamSizeModeWrapper.dataset.value || 'custom'}`;
      } else if (payload.type === 'error') {
        renderStageStatus.classList.remove('is-live');
        renderStageStatus.classList.add('is-error');
        const detail = payload.message || payload.code || '未知错误';
        renderStageStatusText.textContent =
          `Blender 帧流不可用：${String(detail).slice(0, 80)}`;
        blenderDiagnosticOutput.textContent =
          `实时画面：${detail}`;
      }
    };
    socket.onerror = () => {
      renderStageStatus.classList.add('is-error');
      renderStageStatusText.textContent = 'Blender 实时流连接失败';
    };
    socket.onclose = () => {
      if (blenderStreamSocket === socket) blenderStreamSocket = null;
      if (generation !== blenderStreamGeneration || renderSource !== 'blender') return;
      renderStageStatus.classList.remove('is-live');
      renderStageStatusText.textContent = 'Blender 实时流正在重连';
      blenderStreamReconnectAttempts += 1;
      const reconnectDelay = Math.min(
        15000,
        1000 * (2 ** Math.min(blenderStreamReconnectAttempts - 1, 4))
      );
      blenderStreamReconnectTimer = window.setTimeout(
        () => startBlenderStream(),
        reconnectDelay
      );
    };
  }

  function connectInteractionStream() {
    if (
      interactionStreamSocket
      && (interactionStreamSocket.readyState === WebSocket.OPEN
        || interactionStreamSocket.readyState === WebSocket.CONNECTING)
    ) {
      return;
    }
    const socket = new WebSocket(websocketUrl('/api/interactions/stream'));
    interactionStreamSocket = socket;
    socket.onopen = () => {
      interactionStreamConnected = true;
    };
    socket.onmessage = (message) => {
      let payload;
      try {
        payload = JSON.parse(message.data);
      } catch (_) {
        return;
      }
      const event = payload.event;
      if (!event) return;
      if (event.kind === 'blender_proposal') {
        if (!chatRequestPending) loadBlenderProposals();
        return;
      }
      if (event.kind === 'behavior_status') {
        const labels = {
          planning: '正在分析角色动作',
          dispatched: '角色动作已下发',
          unavailable: '角色动作不可用，文本聊天不受影响',
          superseded: '旧动作分析已被更新的消息取消'
        };
        blenderDiagnosticOutput.textContent =
          labels[event.value] || `角色控制：${event.value}`;
        if (event.value === 'dispatched' && !chatRequestPending) {
          loadBlenderProposals();
        }
        return;
      }
      if (renderSource !== '2d') return;
      if (event.kind === 'expression') {
        apply2dControls({ emotes: [event.value], motions: [] });
      } else if (event.kind === 'motion') {
        apply2dControls({ emotes: [], motions: [event.value] });
      }
    };
    socket.onclose = () => {
      if (interactionStreamSocket === socket) interactionStreamSocket = null;
      interactionStreamConnected = false;
      if (interactionReconnectTimer) window.clearTimeout(interactionReconnectTimer);
      interactionReconnectTimer = window.setTimeout(connectInteractionStream, 1200);
    };
    socket.onerror = () => {
      interactionStreamConnected = false;
    };
  }

  async function refreshBlenderStatus() {
    try {
      const bridgeResponse = await fetch('/api/blender/status');
      const bridge = await bridgeResponse.json();
      blenderBridgeStatus.textContent = bridge.reachable
        ? `在线 · ${bridge.address}`
        : (bridge.enabled ? '不可达' : '已关闭');
    } catch (_) {
      blenderBridgeStatus.textContent = '状态读取失败';
    }

    try {
      const previewResponse = await fetch('/api/blender/preview/status');
      if (!previewResponse.ok) {
        throw new Error(await apiErrorMessage(previewResponse, '伴生扩展未连接'));
      }
      const preview = await previewResponse.json();
      const previewError = String(preview.preview?.last_error || '').trim();
      const addonVersion = String(preview.preview?.version || '0.0.0');
      const currentAddon = versionAtLeast(addonVersion, '0.5.5');
      blenderPreviewStatus.textContent = preview.preview?.available && !previewError
        ? (currentAddon
            ? `v${addonVersion} · 可用`
            : `v${addonVersion} · 请更新至 v0.5.5`)
        : (preview.preview?.available ? '已加载 · 帧错误' : '不可用');
      const cameraSize = preview.preview?.camera_resolution;
      blenderCameraStatus.textContent = Array.isArray(cameraSize)
        ? `${cameraSize[0]}×${cameraSize[1]}`
        : (preview.preview?.active_camera
            ? `${preview.preview.active_camera} · 需 v0.3`
            : '无活动相机');
      const streamSize = preview.stream?.resolved_size;
      if (Array.isArray(streamSize) && streamSize.length === 2) {
        lastRenderFrameSize = {
          width: Number(streamSize[0]) || 512,
          height: Number(streamSize[1]) || 512
        };
        if (!renderPaneManuallySized) syncRenderPaneToFrame();
      }
      blenderStreamStatus.textContent = Array.isArray(streamSize)
        ? `${streamSize[0]}×${streamSize[1]} · ${preview.stream?.size_mode || 'custom'}`
        : '未解析';
      if (previewError) {
        blenderDiagnosticOutput.textContent = `画面扩展：${previewError}`;
      } else if (!currentAddon) {
        blenderDiagnosticOutput.textContent =
          `当前扩展 v${addonVersion} 不包含语义手臂骨链、语义 IK 坐标或可靠的 Blender 4.5 相机帧传输。请从项目 blender_addons 安装 v0.5.5 并重新启用扩展。`;
      } else {
        try {
          blenderDiagnosticOutput.textContent = await describeBlenderRigCapabilities();
        } catch (error) {
          blenderDiagnosticOutput.textContent = `骨架能力：${error.message}`;
        }
      }
    } catch (error) {
      blenderPreviewStatus.textContent = '未安装或未启用';
      blenderCameraStatus.textContent = '不可用';
      blenderStreamStatus.textContent = '不可用';
      blenderDiagnosticOutput.textContent = `画面扩展：${error.message}`;
    }
  }

  async function describeBlenderRigCapabilities() {
    const response = await fetch('/api/blender/capabilities');
    if (!response.ok) {
      throw new Error(await apiErrorMessage(response, '骨架能力读取失败'));
    }
    const payload = await response.json();
    const generated = payload.capabilities?.generated_behavior || {};
    const rig = generated.rig_profile || {};
    const limbLabels = {
      arm_left: '左臂',
      arm_right: '右臂',
      leg_left: '左腿',
      leg_right: '右腿'
    };
    const limbs = Object.entries(rig.limbs || {}).map(([name, value]) => {
      const mode = value.active_channel || value.mode || 'unknown';
      const safety = value.selection_ambiguous
        ? '约束冲突'
        : (value.safe === false ? '受限' : '可控');
      return `${limbLabels[name] || name}:${mode}/${safety}`;
    });
    const readiness = rig.safety?.full_body_ready ? '全身通道就绪' : '按安全通道降级';
    const signature = String(rig.signature || '').slice(0, 12) || '未生成';
    const adapter = rig.mmd_detected ? 'PMX/mmd_tools' : 'Blender Armature';
    const profileVersion = Number(rig.version) || 0;
    const cacheState = payload.cache?.hit ? '缓存命中' : '已重新扫描';
    return `RigProfile v${profileVersion} ${signature} · ${adapter} · ${readiness} · ${limbs.join('，') || '未识别四肢'} · ${cacheState}`;
  }

  function versionAtLeast(actual, required) {
    const parse = value => String(value).split('.').map(part => Number(part) || 0);
    const left = parse(actual);
    const right = parse(required);
    for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
      const difference = (left[index] || 0) - (right[index] || 0);
      if (difference !== 0) return difference > 0;
    }
    return true;
  }

  function apply2dControls(controls = {}) {
    const emote = Array.isArray(controls.emotes) ? controls.emotes[0] : null;
    const motion = Array.isArray(controls.motions) ? controls.motions[0] : null;
    const fxMap = {
      happy: '✦',
      shy: '♡',
      surprised: '!',
      angry: '⚡',
      sad: '…',
      wink: '✧',
      blink: '·'
    };
    if (emote) {
      const previousEmotes = Array.from(characterAvatar.classList)
        .filter(name => name.startsWith('is-emote-'));
      characterAvatar.classList.remove(...previousEmotes);
      characterStageFx.textContent = fxMap[emote] || '';
      if (emoteResetTimer) window.clearTimeout(emoteResetTimer);
      void characterAvatar.offsetWidth;
      characterAvatar.classList.add(`is-emote-${emote}`);
      emoteResetTimer = window.setTimeout(() => {
        characterAvatar.classList.remove(`is-emote-${emote}`);
        characterStageFx.textContent = '';
      }, 1800);
    }
    if (motion) {
      const previousMotions = Array.from(characterAvatar.classList)
        .filter(name => name.startsWith('is-motion-'));
      characterAvatar.classList.remove(...previousMotions);
      if (motionResetTimer) window.clearTimeout(motionResetTimer);
      void characterAvatar.offsetWidth;
      characterAvatar.classList.add(`is-motion-${motion}`);
      motionResetTimer = window.setTimeout(() => {
        characterAvatar.classList.remove(`is-motion-${motion}`);
      }, 1800);
    }
  }

  function releaseActiveAudio() {
    if (!activeAudio) return;
    activeAudio.pause();
    activeAudio.removeAttribute('src');
    activeAudio.load();
    activeAudio = null;
  }

  function attachReplayControl(bubble) {
    if (!bubble || bubble.querySelector('.bubble-audio-btn')) return;
    const content = bubble.querySelector('.bubble-content');
    if (!content) return;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'bubble-audio-btn';
    button.title = '重新合成并朗读这条消息';
    button.setAttribute('aria-label', '重新朗读这条消息');
    button.addEventListener('click', async () => {
      button.disabled = true;
      try {
        await speakText(content.textContent || '', true);
        ttsStatusText.textContent = '历史消息已重新发起语音合成。';
      } catch (error) {
        ttsStatusText.textContent = `历史消息朗读失败：${error.message}`;
      } finally {
        button.disabled = false;
      }
    });
    bubble.appendChild(button);
  }

  async function speakText(text, force = false) {
    const provider = ttsProviderSelectWrapper.dataset.value || 'none';
    if (provider === 'none') {
      if (force) throw new Error('请先选择 TTS Provider');
      return;
    }
    if (provider === 'browser') {
      if (!('speechSynthesis' in window)) {
        throw new Error('当前浏览器不支持 SpeechSynthesis');
      }
      releaseActiveAudio();
      window.speechSynthesis.cancel();
      const utterance = new SpeechSynthesisUtterance(text);
      utterance.lang = /[\u3400-\u9fff]/.test(text) ? 'zh-CN' : 'en-US';
      utterance.rate = Math.min(4, Math.max(0.25, Number(ttsSpeed.value) || 1));
      const requestedVoice = ttsVoice.value.trim().toLowerCase();
      if (requestedVoice) {
        const voice = window.speechSynthesis.getVoices().find(item =>
          item.name.toLowerCase().includes(requestedVoice)
        );
        if (voice) utterance.voice = voice;
      }
      window.speechSynthesis.speak(utterance);
      return;
    }
    if ('speechSynthesis' in window) window.speechSynthesis.cancel();
    const response = await fetch('/api/tts/synthesize', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text })
    });
    if (!response.ok) {
      throw new Error(await apiErrorMessage(response, 'TTS 合成失败'));
    }
    const audioData = await response.json();
    releaseActiveAudio();
    const audio = new Audio(audioData.data_url);
    activeAudio = audio;
    const release = () => {
      if (activeAudio === audio) releaseActiveAudio();
    };
    audio.addEventListener('ended', release, { once: true });
    audio.addEventListener('error', release, { once: true });
    await audio.play();
  }

  function syncRenderPaneToFrame() {
    if (!viewportMainBody.classList.contains('split-render-mode')) return;
    const bodyRect = viewportMainBody.getBoundingClientRect();
    const paneHeight = renderingWindowPane.getBoundingClientRect().height || bodyRect.height;
    const width = Math.max(1, Number(lastRenderFrameSize.width) || 512);
    const height = Math.max(1, Number(lastRenderFrameSize.height) || 512);
    const aspectWidth = paneHeight * (width / height);
    const maxWidth = bodyRect.width * 0.75;
    const resolvedWidth = Math.min(maxWidth, Math.max(180, aspectWidth));
    renderingWindowPane.style.width = `${Math.round(resolvedWidth)}px`;
    document.documentElement.style.setProperty(
      '--render-pane-width',
      `${Math.round(resolvedWidth)}px`
    );
  }

  // Draggable Resizer Bar & Expand/Collapse Toggle
  compactAvatarBadge.addEventListener('click', () => {
    if (viewportMainBody.classList.contains('split-render-mode')) {
      viewportMainBody.classList.remove('split-render-mode');
      renderingWindowPane.style.width = '0px';
    } else {
      viewportMainBody.classList.add('split-render-mode');
      renderPaneManuallySized = false;
      window.requestAnimationFrame(syncRenderPaneToFrame);
    }
  });

  let isDraggingResizer = false;

  paneResizerHandle.addEventListener('mousedown', (e) => {
    isDraggingResizer = true;
    paneResizerHandle.classList.add('dragging');
    document.body.style.cursor = 'col-resize';
    e.preventDefault();
  });

  window.addEventListener('mousemove', (e) => {
    if (!isDraggingResizer) return;
    const bodyRect = viewportMainBody.getBoundingClientRect();
    let newWidth = e.clientX - bodyRect.left;
    const maxWidth = bodyRect.width * 0.75;

    if (newWidth < 80) {
      newWidth = 0;
    } else if (newWidth > maxWidth) {
      newWidth = maxWidth;
    }

    renderingWindowPane.style.width = `${newWidth}px`;
    document.documentElement.style.setProperty('--render-pane-width', `${newWidth}px`);
  });

  window.addEventListener('mouseup', () => {
    if (isDraggingResizer) {
      isDraggingResizer = false;
      paneResizerHandle.classList.remove('dragging');
      document.body.style.cursor = 'default';

      const currentWidth = parseFloat(renderingWindowPane.style.width || '0');
      if (currentWidth <= 80) {
        viewportMainBody.classList.remove('split-render-mode');
        renderingWindowPane.style.width = '0px';
      } else {
        renderPaneManuallySized = true;
      }
    }
  });

  window.addEventListener('resize', () => {
    if (!renderPaneManuallySized) window.requestAnimationFrame(syncRenderPaneToFrame);
  });

  // Open Top-Level Glass Settings Overlay
  threeDotsBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    glassSettingsOverlay.classList.add('active');
    loadActiveMemoryData();
    loadPersonaCards();
  });

  closeSettingsPanel.addEventListener('click', () => {
    glassSettingsOverlay.classList.remove('active');
  });

  glassSettingsOverlay.addEventListener('click', (e) => {
    if (e.target === glassSettingsOverlay) {
      glassSettingsOverlay.classList.remove('active');
    }
  });

  // Pure Text Tab Navigation
  tabBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      tabBtns.forEach(b => b.classList.remove('active'));
      tabPanes.forEach(p => p.classList.remove('active'));

      btn.classList.add('active');
      const tabId = btn.getAttribute('data-tab');
      const targetPane = document.getElementById(tabId);
      if (targetPane) {
        targetPane.classList.add('active');
      }

      if (tabId === 'tabMemory') {
        loadActiveMemoryData();
      } else if (tabId === 'tabPersona') {
        loadPersonaCards();
      } else if (tabId === 'tabBlender') {
        refreshBlenderStatus();
      }
    });
  });

  // Memory Data Fetcher
  async function loadActiveMemoryData() {
    activeCharMemoryName.textContent = activeCharacterName;
    memoryDetailContainer.replaceChildren();
    const loading = document.createElement('p');
    loading.className = 'loading-text';
    loading.textContent = '正在读取记忆数据……';
    memoryDetailContainer.appendChild(loading);
    try {
      const res = await fetch('/api/memory');
      if (!res.ok) {
        throw new Error(await apiErrorMessage(res, '记忆接口读取失败'));
      }
      const data = await res.json();
      memoryDetailContainer.replaceChildren();
      const title = document.createElement('strong');
      title.textContent = `[${data.character}] 独立 SQLite 记忆条目`;
      memoryDetailContainer.appendChild(title);
      const summary = document.createElement('p');
      const stats = data.stats || {};
      summary.textContent = `消息 ${stats.messages || 0}，事实 ${stats.facts || 0}，情节 ${stats.episodes || 0}；嵌入器 ${data.embedder || 'unknown'}`;
      memoryDetailContainer.appendChild(summary);
      const list = document.createElement('ul');
      list.style.marginTop = '8px';
      list.style.paddingLeft = '18px';
      const facts = Array.isArray(data.facts) ? data.facts : [];
      if (facts.length === 0) {
        const empty = document.createElement('li');
        empty.textContent = '尚无提取出的长期事实；对话消息已按角色隔离保存。';
        list.appendChild(empty);
      } else {
        facts.forEach(fact => {
          const item = document.createElement('li');
          item.style.marginBottom = '4px';
          item.textContent = `[事实] ${fact.fact}（重要度 ${Number(fact.importance).toFixed(1)}）`;
          list.appendChild(item);
        });
      }
      memoryDetailContainer.appendChild(list);
    } catch (error) {
      memoryDetailContainer.replaceChildren();
      const message = document.createElement('p');
      message.textContent = `记忆读取失败：${error.message}`;
      memoryDetailContainer.appendChild(message);
    }
  }

  // Persona Cards Loader
  async function loadPersonaCards() {
    try {
      const res = await fetch('/api/characters');
      if (res.ok) {
        const data = await res.json();
        renderPersonaGrid(data.characters || []);
        if (Array.isArray(data.errors) && data.errors.length > 0) {
          showTemporaryNoticePill(`有 ${data.errors.length} 张角色卡无法加载`);
        }
      } else {
        throw new Error(await apiErrorMessage(res, '角色卡接口返回异常'));
      }
    } catch (error) {
      personaSettingsGrid.replaceChildren();
      const message = document.createElement('p');
      message.textContent = `角色卡读取失败：${error.message}`;
      personaSettingsGrid.appendChild(message);
    }
  }

  function renderPersonaGrid(characters) {
    personaSettingsGrid.replaceChildren();
    characters.forEach(c => {
      const nameZh = (c.display_name && c.display_name.zh) ? c.display_name.zh : c.name;
      const card = document.createElement('div');
      card.className = `theme-option-card ${c.name === activeCharacterName ? 'active' : ''}`;
      card.style.flexDirection = 'column';
      card.style.alignItems = 'flex-start';
      const title = document.createElement('div');
      title.style.fontWeight = 'bold';
      title.style.color = 'inherit';
      title.textContent = `🎭 ${nameZh} (${c.name})`;
      const personality = document.createElement('div');
      personality.style.fontSize = '12px';
      personality.style.opacity = '0.75';
      personality.style.marginTop = '4px';
      personality.textContent = c.personality || '';
      const memory = document.createElement('div');
      memory.style.fontSize = '11px';
      memory.style.color = '#60a5fa';
      memory.style.marginTop = '6px';
      memory.textContent = `独立记忆：data/memories/${c.name.toLowerCase()}.db`;
      card.append(title, personality, memory);
      card.addEventListener('click', () => switchActiveCharacter(c.card_file, c.name, nameZh));
      personaSettingsGrid.appendChild(card);
    });
  }

  async function switchActiveCharacter(cardFile, name, nameZh) {
    try {
      const response = await fetch('/api/characters/switch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ card_file: cardFile })
      });
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, '角色切换失败'));
      }
      const data = await response.json();
      activeCharacterName = data.active_character || name;
      activePersonaAvatarUrl = data.avatar
        ? `/api/persona/avatar?v=${Date.now()}`
        : '';
      if (!hasCustomAvatar) {
        applyPersonaAvatarImage(activePersonaAvatarUrl);
      }
      activeGreeting = stripControlTags(data.greeting)
        || `你好，我是 ${activeCharacterName}。`;
      updateAbstractHeaderPill();
      await Promise.all([
        loadPersonaCards(),
        loadActiveMemoryData(),
        loadChatHistory(true)
      ]);
      await loadBlenderProposals();
      showTemporaryNoticePill(`已切换至 ${nameZh} (${activeCharacterName})`);
    } catch (error) {
      showTemporaryNoticePill(`角色切换失败：${error.message}`);
    }
  }

  // Preset Theme Selection
  document.querySelectorAll('.theme-option-card').forEach(card => {
    card.addEventListener('click', () => {
      const theme = card.getAttribute('data-bg');
      if (theme) {
        localStorage.removeItem('main_scene_backdrop_bg');
        document.documentElement.style.removeProperty('--main-scene-backdrop');
        document.documentElement.style.removeProperty('--desktop-loop-bg');
        if (!hasCustomBackground) {
          desktopBlurredBg.style.backgroundImage = 'none';
          desktopBlurredBg.style.background = 'var(--desktop-loop-bg)';
        }

        setThemePreset(theme);
      }
    });
  });

  function setThemePreset(theme) {
    document.body.setAttribute('data-theme', theme);
    localStorage.setItem('ui_bg_theme', theme);

    if (presetThemeMap[theme]) {
      updateExtractedColorString(presetThemeMap[theme].rgb);
      mainBackdropColorPicker.value = presetThemeMap[theme].hex;
      mainColorHexText.textContent = presetThemeMap[theme].hex.toUpperCase();

      updateSettingsPanelContrast(presetThemeMap[theme].hex);
    }

    document.querySelectorAll('.theme-option-card').forEach(c => {
      if (c.getAttribute('data-bg') === theme) {
        c.classList.add('active');
      } else {
        c.classList.remove('active');
      }
    });
  }

  async function uploadUiAsset(kind, file) {
    const response = await fetch(`/api/ui/assets/${kind}`, {
      method: 'PUT',
      headers: { 'Content-Type': file.type || 'image/png' },
      body: file
    });
    if (!response.ok) {
      throw new Error(await apiErrorMessage(response, '图片保存失败'));
    }
    return response.json();
  }

  async function restoreUiAssets(assetState) {
    const legacyBackground = localStorage.getItem('ui_custom_bg_img');
    const legacyAvatar = localStorage.getItem('ui_custom_avatar_img');
    if (assetState.background) {
      applyCustomBgImage(`/api/ui/assets/background?v=${Date.now()}`);
    } else if (legacyBackground?.startsWith('data:image/')) {
      try {
        const blob = await fetch(legacyBackground).then(response => response.blob());
        const stored = await uploadUiAsset('background', blob);
        applyCustomBgImage(stored.url);
        localStorage.removeItem('ui_custom_bg_img');
      } catch (error) {
        showTemporaryNoticePill(`旧背景迁移失败：${error.message}`);
      }
    }
    if (assetState.avatar) {
      applyCustomAvatarImage(`/api/ui/assets/avatar?v=${Date.now()}`);
    } else if (legacyAvatar?.startsWith('data:image/')) {
      try {
        const blob = await fetch(legacyAvatar).then(response => response.blob());
        const stored = await uploadUiAsset('avatar', blob);
        applyCustomAvatarImage(stored.url);
        localStorage.removeItem('ui_custom_avatar_img');
      } catch (error) {
        showTemporaryNoticePill(`旧头像迁移失败：${error.message}`);
        applyPersonaAvatarImage(activePersonaAvatarUrl);
      }
    } else {
      applyPersonaAvatarImage(activePersonaAvatarUrl);
    }
  }

  // Images are streamed to project files; no base64 data is retained in localStorage.
  bgFileInput.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (file) {
      try {
        const stored = await uploadUiAsset('background', file);
        applyCustomBgImage(stored.url);
        localStorage.removeItem('ui_custom_bg_img');
        showTemporaryNoticePill(`背景已保存到项目文件（${(stored.bytes / 1048576).toFixed(1)} MB）`);
      } catch (error) {
        showTemporaryNoticePill(error.message);
      } finally {
        bgFileInput.value = '';
      }
    }
  });

  resetBgBtn.addEventListener('click', async () => {
    await fetch('/api/ui/assets/background', { method: 'DELETE' }).catch(() => {});
    hasCustomBackground = false;
    mainBackdropImageLayer.style.display = 'none';
    desktopBlurredBg.style.backgroundImage = 'none';
    localStorage.removeItem('ui_custom_bg_img');
    localStorage.removeItem('main_scene_backdrop_bg');
    document.documentElement.style.removeProperty('--main-scene-backdrop');
    document.documentElement.style.removeProperty('--desktop-loop-bg');
    setThemePreset('warm-beige');
  });

  function applyCustomBgImage(dataUrl) {
    hasCustomBackground = true;
    mainBackdropImageLayer.style.backgroundImage = `url("${dataUrl}")`;
    mainBackdropImageLayer.style.display = 'block';
    desktopBlurredBg.style.backgroundImage = `url("${dataUrl}")`;
    desktopBlurredBg.style.backgroundSize = 'cover';
    sampleImageColorAndSetGlow(dataUrl);
  }

  function sampleImageColorAndSetGlow(dataUrl) {
    const img = new Image();
    img.crossOrigin = "Anonymous";
    img.onload = () => {
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      canvas.width = 60;
      canvas.height = 60;
      ctx.drawImage(img, 0, 0, 60, 60);
      const data = ctx.getImageData(5, 5, 50, 50).data;
      let r = 0, g = 0, b = 0, count = 0;
      for (let i = 0; i < data.length; i += 4) {
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
        count++;
      }
      r = Math.round(r / count);
      g = Math.round(g / count);
      b = Math.round(b / count);

      updateExtractedColorString(`${r}, ${g}, ${b}`);
      updateWindowControlContrast(
        `#${[r, g, b].map(value => value.toString(16).padStart(2, '0')).join('')}`
      );
    };
    img.src = dataUrl;
  }

  function updateExtractedColorString(rgbStr) {
    document.documentElement.style.setProperty('--glow-color-rgb', rgbStr);
    const stroke = document.getElementById('brightAccentStroke');
    if (stroke) {
      stroke.style.borderColor = `rgba(${rgbStr}, 0.85)`;
      stroke.style.boxShadow = `0 0 22px rgba(${rgbStr}, 0.55), inset 0 0 10px rgba(${rgbStr}, 0.25)`;
    }
  }

  avatarFileInput.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    if (file) {
      try {
        const stored = await uploadUiAsset('avatar', file);
        applyCustomAvatarImage(stored.url);
        localStorage.removeItem('ui_custom_avatar_img');
        showTemporaryNoticePill(`立绘已保存到项目文件（${(stored.bytes / 1048576).toFixed(1)} MB）`);
      } catch (error) {
        showTemporaryNoticePill(error.message);
      } finally {
        avatarFileInput.value = '';
      }
    }
  });

  resetAvatarBtn.addEventListener('click', async () => {
    await fetch('/api/ui/assets/avatar', { method: 'DELETE' }).catch(() => {});
    hasCustomAvatar = false;
    applyPersonaAvatarImage(activePersonaAvatarUrl);
    localStorage.removeItem('ui_custom_avatar_img');
  });

  function applyCustomAvatarImage(dataUrl) {
    hasCustomAvatar = true;
    applyAvatarImage(dataUrl);
  }

  function applyPersonaAvatarImage(dataUrl) {
    hasCustomAvatar = false;
    if (!dataUrl) {
      showAvatarFallback();
      return;
    }
    applyAvatarImage(dataUrl);
  }

  function applyAvatarImage(dataUrl) {
    avatarSvg.style.display = 'none';
    avatarImg.onerror = showAvatarFallback;
    avatarImg.src = dataUrl;
    avatarImg.style.display = 'block';

    badgeFallbackIcon.style.display = 'none';
    badgeAvatarImg.onerror = () => {
      badgeAvatarImg.style.display = 'none';
      badgeFallbackIcon.style.display = 'block';
    };
    badgeAvatarImg.src = dataUrl;
    badgeAvatarImg.style.display = 'block';
  }

  function showAvatarFallback() {
    avatarImg.removeAttribute('src');
    avatarImg.style.display = 'none';
    avatarSvg.style.display = 'block';
    badgeAvatarImg.removeAttribute('src');
    badgeAvatarImg.style.display = 'none';
    badgeFallbackIcon.style.display = 'block';
  }

  // Main UI Scene & Outer Blurred Loop Sync Color Picker
  mainBackdropColorPicker.addEventListener('input', (e) => {
    const colorHex = e.target.value.toUpperCase();
    mainColorHexText.textContent = colorHex;
    setMainSceneBackdropColor(colorHex);
    localStorage.setItem('main_scene_backdrop_bg', colorHex);
  });

  function setMainSceneBackdropColor(colorHex) {
    document.body.setAttribute('data-theme', 'custom');

    const uppercaseHex = colorHex.toUpperCase();
    mainBackdropColorPicker.value = uppercaseHex;
    mainColorHexText.textContent = uppercaseHex;
    // Use the requested color exactly; atmosphere is supplied by the outer
    // glow and glass layers rather than altering the selected swatch.
    document.documentElement.style.setProperty('--main-scene-backdrop', uppercaseHex);
    document.documentElement.style.setProperty('--desktop-loop-bg', uppercaseHex);

    // 2. Settings Overlay Glass Panel Background & Controls Contrast Sync
    updateSettingsPanelContrast(uppercaseHex);

    // 3. Outer Desktop Blurred Loop Sync
    if (!hasCustomBackground) {
      desktopBlurredBg.style.backgroundImage = 'none';
      desktopBlurredBg.style.background = uppercaseHex;
    }

    // 4. Ambient Glow Halo Sync
    let num = parseInt(uppercaseHex.replace("#", ""), 16);
    let r = (num >> 16);
    let g = (num >> 8 & 0x00FF);
    let b = (num & 0x0000FF);
    updateExtractedColorString(`${r}, ${g}, ${b}`);
  }

  // Independent Swatch Pickers (All uppercase hex display)
  userBubbleColorPicker.addEventListener('input', (e) => {
    const colorHex = e.target.value.toUpperCase();
    userColorHexText.textContent = colorHex;
    setUserBubbleColor(colorHex);
    localStorage.setItem('user_bubble_hex', colorHex);
  });

  function setUserBubbleColor(colorHex) {
    const uppercaseHex = colorHex.toUpperCase();
    userBubbleColorPicker.value = uppercaseHex;
    userColorHexText.textContent = uppercaseHex;
    const textColor = getContrastTextColor(uppercaseHex);
    document.documentElement.style.setProperty('--user-bubble-bg', uppercaseHex);
    document.documentElement.style.setProperty('--user-bubble-color', textColor);
  }

  accentColorPicker.addEventListener('input', (e) => {
    const colorHex = e.target.value.toUpperCase();
    accentColorHexText.textContent = colorHex;
    setAccentColor(colorHex);
    localStorage.setItem('ui_accent_color', colorHex);
  });

  function setAccentColor(colorHex) {
    const uppercaseHex = colorHex.toUpperCase();
    accentColorPicker.value = uppercaseHex;
    accentColorHexText.textContent = uppercaseHex;
    let num = parseInt(uppercaseHex.replace("#", ""), 16);
    let r = (num >> 16);
    let g = (num >> 8 & 0x00FF);
    let b = (num & 0x0000FF);
    const textColor = getContrastTextColor(uppercaseHex);

    document.documentElement.style.setProperty('--accent-color', uppercaseHex);
    document.documentElement.style.setProperty('--accent-color-rgb', `${r}, ${g}, ${b}`);
    document.documentElement.style.setProperty('--pill-text-color', textColor);
  }

  characterBubbleColorPicker.addEventListener('input', (e) => {
    const colorHex = e.target.value.toUpperCase();
    charColorHexText.textContent = colorHex;
    setCharBubbleColor(colorHex);
    localStorage.setItem('char_bubble_bg', colorHex);
  });

  function setCharBubbleColor(colorHex) {
    const uppercaseHex = colorHex.toUpperCase();
    characterBubbleColorPicker.value = uppercaseHex;
    charColorHexText.textContent = uppercaseHex;
    const textColor = getContrastTextColor(uppercaseHex);
    document.documentElement.style.setProperty('--character-bubble-bg', uppercaseHex);
    document.documentElement.style.setProperty('--character-bubble-color', textColor);
  }

  renderBackdropColorPicker.addEventListener('input', (e) => {
    const colorHex = e.target.value.toUpperCase();
    setRenderBackdropColor(colorHex);
    localStorage.setItem('render_backdrop_bg', colorHex);
  });

  function setRenderBackdropColor(colorHex) {
    const uppercaseHex = colorHex.toUpperCase();
    renderBackdropColorPicker.value = uppercaseHex;
    renderColorHexText.textContent = uppercaseHex;
    document.documentElement.style.setProperty('--render-backdrop-bg', uppercaseHex);
  }

  function adjustColorBrightness(hex, percent) {
    let num = parseInt(hex.replace("#", ""), 16);
    let amt = Math.round(2.55 * percent);
    let R = (num >> 16) + amt;
    let G = (num >> 8 & 0x00FF) + amt;
    let B = (num & 0x0000FF) + amt;
    return "#" + (0x1000000 + (R < 255 ? (R < 1 ? 0 : R) : 255) * 0x10000 + (G < 255 ? (G < 1 ? 0 : G) : 255) * 0x100 + (B < 255 ? (B < 1 ? 0 : B) : 255)).toString(16).slice(1).toUpperCase();
  }

  saveLlmBtn.addEventListener('click', async () => {
    const payload = {
      llm_provider: llmProviderSelectWrapper.dataset.value,
      llm_base_url: llmBaseUrl.value.trim(),
      llm_model: llmModel.value.trim(),
      llm_max_tokens: Number(llmMaxTokens.value),
      llm_example_dialogue_limit: Number(llmExampleLimit.value),
      llm_thinking_mode: llmThinkingSelectWrapper.dataset.value || 'disabled'
    };
    if (llmApiKey.value.trim()) {
      payload.llm_api_key = llmApiKey.value.trim();
    }
    try {
      const response = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, '模型配置保存失败'));
      }
      const saved = await response.json();
      llmApiKey.value = '';
      updateApiKeyPlaceholder(llmApiKey, Boolean(saved.api_key_persisted), null);
      const health = await fetch('/api/llm/status');
      if (!health.ok) {
        throw new Error(await apiErrorMessage(health, '配置已保存，但模型连接检测失败'));
      }
      const status = await health.json();
      showTemporaryNoticePill(`LLM 已连接：${status.model}`);
    } catch (error) {
      showTemporaryNoticePill(error.message);
    }
  });

  llmProviderSelectWrapper.addEventListener('custom-select-change', (event) => {
    const preset = llmProviderPresets.get(event.detail.value);
    if (!preset || preset.id === 'custom') return;
    llmBaseUrl.value = preset.base_url || '';
    llmModel.value = preset.default_model || '';
    updateApiKeyPlaceholder(llmApiKey, false, preset);
  });

  ttsProviderSelectWrapper.addEventListener('custom-select-change', (event) => {
    const preset = ttsProviderPresets.get(event.detail.value);
    if (!preset) return;
    ttsBaseUrl.value = preset.base_url || '';
    ttsModel.value = preset.default_model || '';
    ttsVoice.value = preset.default_voice || '';
    setCustomSelectValue(ttsFormatSelectWrapper, preset.default_format || 'mp3');
    updateApiKeyPlaceholder(ttsApiKey, false, preset);
  });

  saveTtsBtn.addEventListener('click', async () => {
    const payload = {
      tts_provider: ttsProviderSelectWrapper.dataset.value || 'none',
      tts_base_url: ttsBaseUrl.value.trim(),
      tts_model: ttsModel.value.trim(),
      tts_voice: ttsVoice.value.trim(),
      tts_response_format: ttsFormatSelectWrapper.dataset.value || 'mp3',
      tts_speed: Number(ttsSpeed.value) || 1,
      tts_style: ttsStyle.value.trim(),
      tts_auto_play: ttsAutoPlay.checked
    };
    if (ttsApiKey.value.trim()) payload.tts_api_key = ttsApiKey.value.trim();
    try {
      const response = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, 'TTS 配置保存失败'));
      }
      const saved = await response.json();
      ttsApiKey.value = '';
      updateApiKeyPlaceholder(ttsApiKey, Boolean(saved.api_key_persisted), null);
      ttsAutoPlayEnabled = ttsAutoPlay.checked;
      const statusResponse = await fetch('/api/tts/status');
      const status = await statusResponse.json();
      ttsStatusText.textContent = status.detail || 'TTS 配置已保存。';
      showTemporaryNoticePill('TTS 配置已保存');
    } catch (error) {
      ttsStatusText.textContent = `TTS 配置失败：${error.message}`;
    }
  });

  testTtsBtn.addEventListener('click', async () => {
    testTtsBtn.disabled = true;
    try {
      await speakText('你好，这是一段 RaViChara 语音合成测试。', true);
      ttsStatusText.textContent = '试听请求已执行。';
    } catch (error) {
      ttsStatusText.textContent = `试听失败：${error.message}`;
    } finally {
      testTtsBtn.disabled = false;
    }
  });

  saveBlenderSettingsBtn.addEventListener('click', async () => {
    const playbackStart = Number(blenderPlaybackStart.value);
    const actionStart = Number(blenderActionStart.value);
    const actionEnd = Number(blenderActionEnd.value);
    const playbackEnd = Number(blenderPlaybackEnd.value);
    const transitionFrames = Number(blenderTransitionFrames.value);
    if (!(playbackStart <= actionStart && actionStart < actionEnd && actionEnd <= playbackEnd)) {
      showTemporaryNoticePill(
        '帧范围必须满足：播放起始 ≤ 动作起始 < 动作结束 ≤ 播放结束'
      );
      return;
    }
    if (
      actionStart - playbackStart < transitionFrames
      || playbackEnd - actionEnd < transitionFrames
    ) {
      showTemporaryNoticePill('播放范围前后必须预留至少“最小过渡帧数”');
      return;
    }
    saveBlenderSettingsBtn.disabled = true;
    try {
      const response = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          blender_stream_size_mode:
            blenderStreamSizeModeWrapper.dataset.value || 'camera',
          blender_stream_width: Number(blenderStreamWidth.value),
          blender_stream_height: Number(blenderStreamHeight.value),
          blender_stream_fps: Number(blenderStreamFps.value),
          blender_playback_start_frame: playbackStart,
          blender_action_start_frame: actionStart,
          blender_action_end_frame: actionEnd,
          blender_playback_end_frame: playbackEnd,
          blender_transition_frames: transitionFrames,
          blender_control_analysis_enabled:
            blenderControlAnalysisEnabled.checked,
          blender_control_analysis_model:
            blenderControlAnalysisModel.value.trim(),
          blender_scene_proposals_enabled:
            blenderSceneProposalsEnabled.checked,
          video_enabled: videoEnabled.checked,
          video_source_type: videoSourceTypeWrapper.dataset.value || 'video',
          video_source_url: videoSourceUrl.value.trim(),
          video_autoplay: videoAutoplay.checked,
          video_loop_playback: videoLoop.checked,
          video_muted: videoMuted.checked
        })
      });
      if (!response.ok) {
        throw new Error(
          await apiErrorMessage(response, 'Blender 画幅与动作设置保存失败')
        );
      }
      videoSettings = {
        enabled: videoEnabled.checked,
        sourceType: videoSourceTypeWrapper.dataset.value || 'video',
        sourceUrl: videoSourceUrl.value.trim(),
        autoplay: videoAutoplay.checked,
        loop: videoLoop.checked,
        muted: videoMuted.checked
      };
      showTemporaryNoticePill('Blender 与外部视频渲染设置已保存');
      await refreshBlenderStatus();
      if (renderSource === 'blender') startBlenderStream(true);
      if (renderSource === 'video') startExternalVideo();
    } catch (error) {
      showTemporaryNoticePill(`Blender 设置失败：${error.message}`);
    } finally {
      saveBlenderSettingsBtn.disabled = false;
    }
  });

  blenderProfileSelectWrapper.addEventListener('custom-select-change', async (event) => {
    const renderMode = event.detail.value;
    try {
      const response = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          blender_enabled: renderMode !== 'off',
          blender_render_mode: renderMode
        })
      });
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, 'Blender 设置保存失败'));
      }
      const health = await fetch('/api/blender/status');
      const status = await health.json();
      showTemporaryNoticePill(status.detail || `Blender 档位：${renderMode}`);
      await refreshBlenderStatus();
    } catch (error) {
      showTemporaryNoticePill(`Blender 设置失败：${error.message}`);
    }
  });

  renderSourceSelectWrapper.addEventListener('custom-select-change', async (event) => {
    applyRenderSource(event.detail.value);
  });

  refreshBlenderFrameBtn.addEventListener('click', () => {
    if (renderSource !== 'blender') {
      applyRenderSource('blender');
    }
    startBlenderStream(true);
  });

  stopBlenderAnimationBtn.addEventListener('click', async () => {
    stopBlenderAnimationBtn.disabled = true;
    try {
      const response = await fetch('/api/blender/animation/stop', {
        method: 'POST'
      });
      if (!response.ok) {
        throw new Error(
          await apiErrorMessage(response, 'Blender 动作停止失败')
        );
      }
      blenderDiagnosticOutput.textContent = '动作已停止，角色已恢复待机状态。';
      await refreshBlenderStatus();
    } catch (error) {
      blenderDiagnosticOutput.textContent = `动作停止失败：${error.message}`;
    } finally {
      stopBlenderAnimationBtn.disabled = false;
    }
  });

  test2dMotionBtn.addEventListener('click', () => {
    applyRenderSource('2d');
    apply2dControls({ emotes: ['happy'], motions: ['wave'] });
    blenderDiagnosticOutput.textContent = '2D 动作测试：happy + wave 已执行。';
  });

  testVideoSourceBtn.addEventListener('click', () => {
    videoSettings = {
      enabled: videoEnabled.checked,
      sourceType: videoSourceTypeWrapper.dataset.value || 'video',
      sourceUrl: videoSourceUrl.value.trim(),
      autoplay: videoAutoplay.checked,
      loop: videoLoop.checked,
      muted: videoMuted.checked
    };
    applyRenderSource('video');
    blenderDiagnosticOutput.textContent = videoSettings.enabled && videoSettings.sourceUrl
      ? '正在连接外部视频源；该播放流程不写入项目缓存。'
      : '请先启用外部视频并填写 HTTP(S) URL。';
  });

  testBlenderBtn.addEventListener('click', async () => {
    testBlenderBtn.disabled = true;
    blenderDiagnosticOutput.textContent = '正在检查场景、模型、表情和动作……';
    try {
      const response = await fetch('/api/blender/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          inspect: true,
          expression: 'happy',
          motion: 'nod'
        })
      });
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, 'Blender 诊断失败'));
      }
      const result = await response.json();
      const summary = {
        status: result.status,
        scene: result.scene_inspect?.ok,
        models: result.model_inspect?.ok,
        expressions: result.expression_inspect?.ok,
        expression: result.expression?.ok,
        motion: result.motion?.ok
      };
      blenderDiagnosticOutput.textContent = JSON.stringify(summary, null, 2);
      await refreshBlenderStatus();
    } catch (error) {
      blenderDiagnosticOutput.textContent = `诊断失败：${error.message}`;
    } finally {
      testBlenderBtn.disabled = false;
    }
  });

  // Send Chat Message
  async function sendMessage() {
    const text = chatInput.value.trim();
    if (!text || chatRequestPending) return;

    const now = Date.now();
    const timeDiffMinutes = (now - lastMessageTime) / (1000 * 60);

    if (timeDiffMinutes >= configuredTimestampIntervalMin || chatHistory.children.length <= 1) {
      const timeStr = new Date(now).toLocaleString('zh-CN', { hour12: false, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' }).replace(/\//g, '-');
      appendTimestampDivider(timeStr);
    }

    appendBubble(text, 'user');
    chatInput.value = '';
    chatHistory.scrollTop = chatHistory.scrollHeight;

    const clientTimeZone =
      Intl.DateTimeFormat().resolvedOptions().timeZone || 'local';
    const nowFormatted =
      `${new Date(now).toLocaleString('zh-CN', { hour12: false })} (${clientTimeZone})`;
    lastMessageTime = now;
    chatRequestPending = true;
    sendBtn.disabled = true;
    const responseBubble = createBubble('', 'character');
    responseBubble.classList.add('is-streaming');
    responseBubble.setAttribute('aria-busy', 'true');
    const responseContent = responseBubble.querySelector('.bubble-content');
    chatHistory.appendChild(responseBubble);
    chatHistory.scrollTop = chatHistory.scrollHeight;

    try {
      const data = await new Promise((resolve, reject) => {
        const socket = new WebSocket(websocketUrl('/api/chat/stream'));
        let streamedText = '';
        let settled = false;
        const finish = (callback, value) => {
          if (settled) return;
          settled = true;
          socket.onclose = null;
          socket.close();
          callback(value);
        };
        socket.onopen = () => {
          socket.send(JSON.stringify({
            message: text,
            client_timestamp: nowFormatted,
            elapsed_minutes: Math.round(timeDiffMinutes)
          }));
        };
        socket.onmessage = message => {
          let event;
          try {
            event = JSON.parse(message.data);
          } catch (error) {
            finish(reject, new Error(`无法解析模型流：${error.message}`));
            return;
          }
          const payload = event?.data || {};
          if (event.type === 'delta') {
            streamedText += String(payload.content || '');
            responseContent.textContent = stripControlTags(streamedText);
            chatHistory.scrollTop = chatHistory.scrollHeight;
          } else if (event.type === 'done') {
            const result = payload.result;
            if (result?.reply) {
              finish(resolve, result);
            } else {
              finish(reject, new Error('模型流结束但未返回完整回复'));
            }
          } else if (event.type === 'error') {
            finish(
              reject,
              new Error(payload.message || payload.code || '模型流式请求失败')
            );
          }
        };
        socket.onerror = () => {
          finish(reject, new Error('无法建立模型流式连接'));
        };
        socket.onclose = () => {
          if (!settled) finish(reject, new Error('模型流式连接提前关闭'));
        };
      });

      responseContent.textContent = stripControlTags(data.reply);
      responseBubble.classList.remove('is-streaming');
      responseBubble.removeAttribute('aria-busy');
      renderBlenderProposals(data.blender_proposals || []);
      loadedChatMessages.push(
          {
            id: `live-user-${now}`,
            role: 'user',
            content: text,
            timestamp: new Date(now).toISOString()
          },
          {
            id: `live-character-${now}`,
            role: 'character',
            content: data.reply,
            timestamp: data.server_timestamp || new Date().toISOString()
          }
      );
      if (renderSource === '2d' && !interactionStreamConnected) {
        apply2dControls(data.controls || {});
      }
      if (ttsAutoPlayEnabled) {
        speakText(data.reply).catch(error => {
          ttsStatusText.textContent = `自动朗读失败：${error.message}`;
        });
      }
      const tokenSummary = data.usage?.total_tokens
        ? ` · ${data.usage.total_tokens} tokens`
        : '';
      showTemporaryNoticePill(`[${data.character}] 对话已记录${tokenSummary}`);
    } catch (err) {
      responseBubble.classList.remove('is-streaming');
      responseBubble.removeAttribute('aria-busy');
      if (responseContent.textContent.trim()) {
        responseContent.textContent += `\n\n[响应中断] ${err.message}`;
        showTemporaryNoticePill(`对话中断：${err.message}`);
      } else {
        responseBubble.remove();
        showChatError(err.message);
      }
    } finally {
      chatRequestPending = false;
      sendBtn.disabled = false;
    }
  }

  function showChatError(message) {
    appendBubble(`[连接失败] ${message}`, 'character');
    showTemporaryNoticePill(`对话失败：${message}`);
  }

  function createBubble(content, role) {
    const bubble = document.createElement('div');
    bubble.className = `chat-bubble ${role === 'user' ? 'user-bubble' : 'character-bubble'}`;
    const inner = document.createElement('div');
    inner.className = 'bubble-content';
    inner.textContent = role === 'user' ? content : stripControlTags(content);
    bubble.appendChild(inner);
    if (role !== 'user') attachReplayControl(bubble);
    return bubble;
  }

  function appendBubble(content, role) {
    const bubble = createBubble(content, role);
    chatHistory.appendChild(bubble);
    chatHistory.scrollTop = chatHistory.scrollHeight;
  }

  async function loadBlenderProposals() {
    try {
      const response = await fetch('/api/blender/proposals');
      if (!response.ok) return;
      const data = await response.json();
      renderBlenderProposals(data.proposals || []);
    } catch (_) {
      // Proposal discovery is optional and must not block chat startup.
    }
  }

  function renderBlenderProposals(proposals) {
    if (!Array.isArray(proposals)) return;
    proposals.forEach(proposal => {
      if (!proposal?.id) return;
      const alreadyRendered = Array.from(
        chatHistory.querySelectorAll('.blender-proposal-card')
      ).some(card => card.dataset.proposalId === proposal.id);
      if (alreadyRendered) return;

      const card = document.createElement('div');
      card.className = 'blender-proposal-card';
      card.dataset.proposalId = proposal.id;

      const title = document.createElement('strong');
      title.textContent = 'Blender 修改待确认';
      const summary = document.createElement('p');
      summary.textContent = proposal.summary || proposal.kind || '场景修改';
      const metadata = document.createElement('small');
      metadata.textContent = `${proposal.kind || 'change'} · ${proposal.risk || 'persistent'} · 仅保存在内存中`;

      const actions = document.createElement('div');
      actions.className = 'blender-proposal-actions';
      const approve = document.createElement('button');
      approve.type = 'button';
      approve.className = 'btn-primary';
      approve.textContent = '确认执行';
      const decline = document.createElement('button');
      decline.type = 'button';
      decline.className = 'btn-secondary';
      decline.textContent = '拒绝';

      const decide = async approved => {
        approve.disabled = true;
        decline.disabled = true;
        try {
          const response = await fetch(
            `/api/blender/proposals/${encodeURIComponent(proposal.id)}`,
            {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ approve: approved })
            }
          );
          if (!response.ok) {
            throw new Error(
              await apiErrorMessage(response, 'Blender 修改处理失败')
            );
          }
          const result = await response.json();
          card.classList.add('is-resolved');
          actions.replaceChildren();
          const state = document.createElement('span');
          state.textContent = result.executed
            ? '已在 Blender 中执行'
            : '已拒绝，未修改 Blender';
          actions.appendChild(state);
          if (result.executed) refreshBlenderStatus();
        } catch (error) {
          approve.disabled = false;
          decline.disabled = false;
          showTemporaryNoticePill(`Blender 修改失败：${error.message}`);
        }
      };
      approve.addEventListener('click', () => decide(true));
      decline.addEventListener('click', () => decide(false));
      actions.append(approve, decline);
      card.append(title, summary, metadata, actions);
      chatHistory.appendChild(card);
    });
    if (proposals.length) {
      chatHistory.scrollTop = chatHistory.scrollHeight;
    }
  }

  function appendTimestampDivider(text) {
    const div = document.createElement('div');
    div.className = 'chat-timestamp-divider';
    div.textContent = text;
    chatHistory.appendChild(div);
  }

  function parseHistoryTimestamp(value) {
    if (!value) return null;
    const normalized = String(value).includes('T')
      ? String(value)
      : `${String(value).replace(' ', 'T')}Z`;
    const timestamp = new Date(normalized);
    return Number.isNaN(timestamp.getTime()) ? null : timestamp;
  }

  function formatHistoryTimestamp(timestamp) {
    return timestamp.toLocaleString('zh-CN', {
      hour12: false,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    }).replace(/\//g, '-');
  }

  function renderPersistedChatHistory(scrollToBottom) {
    chatHistory.replaceChildren();
    if (chatHistoryHasMore && chatHistoryBeforeId) {
      const loadMore = document.createElement('button');
      loadMore.type = 'button';
      loadMore.className = 'btn-secondary chat-history-load-more';
      loadMore.textContent = '加载更早记录';
      loadMore.addEventListener('click', async () => {
        loadMore.disabled = true;
        await loadChatHistory(false);
      });
      chatHistory.appendChild(loadMore);
    }

    let previousTimestamp = null;
    loadedChatMessages.forEach(message => {
      const timestamp = parseHistoryTimestamp(message.timestamp);
      if (
        timestamp
        && (
          !previousTimestamp
          || (timestamp.getTime() - previousTimestamp.getTime())
            >= configuredTimestampIntervalMin * 60 * 1000
        )
      ) {
        appendTimestampDivider(formatHistoryTimestamp(timestamp));
      }
      if (timestamp) previousTimestamp = timestamp;
      chatHistory.appendChild(createBubble(
        message.content,
        message.role === 'user' ? 'user' : 'character'
      ));
    });

    if (loadedChatMessages.length === 0) {
      appendTimestampDivider(formatHistoryTimestamp(new Date()));
      chatHistory.appendChild(createBubble(
        activeGreeting || `你好，我是 ${activeCharacterName}。`,
        'character'
      ));
      lastMessageTime = Date.now();
    } else {
      const latest = parseHistoryTimestamp(
        loadedChatMessages[loadedChatMessages.length - 1].timestamp
      );
      if (latest) lastMessageTime = latest.getTime();
    }
    if (scrollToBottom) chatHistory.scrollTop = chatHistory.scrollHeight;
  }

  async function loadChatHistory(reset) {
    const previousHeight = chatHistory.scrollHeight;
    const query = new URLSearchParams({ limit: '200' });
    if (!reset && chatHistoryBeforeId) {
      query.set('before_id', String(chatHistoryBeforeId));
    }
    try {
      const response = await fetch(`/api/chat/history?${query}`);
      if (!response.ok) {
        throw new Error(await apiErrorMessage(response, '历史记录读取失败'));
      }
      const data = await response.json();
      const page = Array.isArray(data.messages) ? data.messages : [];
      if (reset) {
        loadedChatMessages = page;
      } else {
        const knownIds = new Set(loadedChatMessages.map(message => message.id));
        loadedChatMessages = page
          .filter(message => !knownIds.has(message.id))
          .concat(loadedChatMessages);
      }
      chatHistoryHasMore = Boolean(data.has_more);
      chatHistoryBeforeId = data.next_before_id || null;
      renderPersistedChatHistory(Boolean(reset));
      if (!reset) {
        chatHistory.scrollTop = Math.max(0, chatHistory.scrollHeight - previousHeight);
      }
    } catch (error) {
      if (reset) {
        loadedChatMessages = [];
        chatHistoryHasMore = false;
        chatHistoryBeforeId = null;
        renderPersistedChatHistory(true);
      }
      showTemporaryNoticePill(`历史记录读取失败：${error.message}`);
    }
  }

  function updateAbstractHeaderPill() {
    const now = new Date();
    const month = String(now.getMonth() + 1).padStart(2, '0');
    const day = String(now.getDate()).padStart(2, '0');
    abstractPillText.textContent = `${activeCharacterName} | ${month}-${day}`;
  }

  function showTemporaryNoticePill(msg) {
    abstractPillText.textContent = `Notice TAB: ${msg}`;
    setTimeout(() => {
      updateAbstractHeaderPill();
    }, 3000);
  }

  sendBtn.addEventListener('click', sendMessage);
  chatInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') sendMessage();
  });

  micBtn.addEventListener('click', () => {
    if ('webkitSpeechRecognition' in window || 'SpeechRecognition' in window) {
      const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
      const recognition = new SpeechRecognition();
      recognition.lang = 'zh-CN';
      recognition.start();

      micBtn.style.color = '#ef4444';
      chatInput.placeholder = "Listening...";

      recognition.onresult = (event) => {
        const transcript = event.results[0][0].transcript;
        chatInput.value = transcript;
        micBtn.style.color = '#52525b';
        chatInput.placeholder = "Type here or hold to speak...";
      };

      recognition.onerror = () => {
        micBtn.style.color = '#52525b';
        chatInput.placeholder = "Type here or hold to speak...";
      };
    } else {
      alert('您的浏览器暂不支持 Web Speech API，请使用文本输入。');
    }
  });

  window.addEventListener('beforeunload', () => {
    stopBlenderStream();
    if (interactionReconnectTimer) window.clearTimeout(interactionReconnectTimer);
    if (interactionStreamSocket) {
      interactionStreamSocket.onclose = null;
      interactionStreamSocket.close();
    }
    if ('speechSynthesis' in window) window.speechSynthesis.cancel();
    releaseActiveAudio();
  });
});
