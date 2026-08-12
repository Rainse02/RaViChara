/* Headless regression checks that require no LLM, TTS, or Blender service. */

const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

let browser;
(async () => {
  const artifactDir = path.resolve('artifacts');
  fs.mkdirSync(artifactDir, { recursive: true });
  browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1366, height: 768 } });
  const pageErrors = [];
  const consoleErrors = [];
  const failedResponses = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') consoleErrors.push(message.text());
  });
  page.on('response', response => {
    if (response.status() >= 400) {
      failedResponses.push({ status: response.status(), url: response.url() });
    }
  });

  await page.goto('http://127.0.0.1:8760', { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle');
  await page.waitForFunction(() => (
    document.querySelector('#activeCharMemoryName')?.textContent === 'Lily'
  ));

  const api = await page.evaluate(async () => {
    const [personaResponse, charactersResponse, settingsResponse, avatarResponse] =
      await Promise.all([
        fetch('/api/persona'),
        fetch('/api/characters'),
        fetch('/api/settings'),
        fetch('/api/persona/avatar')
      ]);
    return {
      persona: await personaResponse.json(),
      characters: await charactersResponse.json(),
      settings: await settingsResponse.json(),
      avatarStatus: avatarResponse.status,
      avatarType: avatarResponse.headers.get('content-type')
    };
  });
  const characterNames = api.characters.characters.map(card => card.name);
  if (
    api.persona.name !== 'Lily'
    || api.persona.card_file !== 'characters/lily.card.yaml'
    || characterNames.length !== 1
    || characterNames[0] !== 'Lily'
    || api.settings.user_bubble_bg !== '#B6EBEC'
    || api.settings.llm.api_key_configured !== false
    || api.avatarStatus !== 200
    || api.avatarType !== 'image/svg+xml'
  ) {
    throw new Error(`distribution defaults are invalid: ${JSON.stringify({
      persona: api.persona,
      characterNames,
      userBubble: api.settings.user_bubble_bg,
      apiKeyConfigured: api.settings.llm.api_key_configured,
      avatarStatus: api.avatarStatus,
      avatarType: api.avatarType
    })}`);
  }

  const visual = await page.evaluate(() => {
    const root = getComputedStyle(document.documentElement);
    const avatar = document.querySelector('#avatarImg');
    return {
      theme: document.body.dataset.theme,
      main: root.getPropertyValue('--main-scene-backdrop').trim().toUpperCase(),
      desktop: root.getPropertyValue('--desktop-loop-bg').trim().toUpperCase(),
      user: root.getPropertyValue('--user-bubble-bg').trim().toUpperCase(),
      accent: root.getPropertyValue('--accent-color').trim().toUpperCase(),
      character: root.getPropertyValue('--character-bubble-bg').trim().toUpperCase(),
      render: root.getPropertyValue('--render-backdrop-bg').trim().toUpperCase(),
      avatarVisible: getComputedStyle(avatar).display !== 'none' && avatar.naturalWidth > 0,
      controlColor: getComputedStyle(document.querySelector('.maximize-btn')).color
    };
  });
  const expected = {
    theme: 'warm-beige',
    main: '#FEF3C7',
    desktop: '#FEF3C7',
    user: '#B6EBEC',
    accent: '#9FF3FE',
    character: '#FFFFFF',
    render: '#BDBBF7',
    avatarVisible: true,
    controlColor: 'rgb(15, 23, 42)'
  };
  if (JSON.stringify(visual) !== JSON.stringify(expected)) {
    throw new Error(`cream theme regression: ${JSON.stringify({ visual, expected })}`);
  }

  const screenshot = path.join(artifactDir, 'ui-offline-regression.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  const unexpectedResponses = failedResponses.filter(response => !(
    response.status === 503
    && (
      response.url.startsWith('http://127.0.0.1:8760/api/blender/frame')
      || response.url === 'http://127.0.0.1:8760/api/blender/preview/status'
    )
  ));
  const unexpectedConsoleErrors = consoleErrors.filter(message => !(
    message.startsWith('Failed to load resource:')
    && failedResponses.some(response => response.status === 503)
  ));
  if (pageErrors.length || unexpectedConsoleErrors.length || unexpectedResponses.length) {
    throw new Error(`browser errors: ${JSON.stringify({
      pageErrors,
      unexpectedConsoleErrors,
      unexpectedResponses
    })}`);
  }
  console.log('RAVICHARA_OFFLINE_UI_TEST=' + JSON.stringify({
    character: api.persona.name,
    palette: visual,
    screenshot
  }));
  await browser.close();
  browser = null;
})().catch(error => {
  console.error(error.stack || error);
  if (browser) browser.close().catch(() => {});
  process.exitCode = 1;
});
