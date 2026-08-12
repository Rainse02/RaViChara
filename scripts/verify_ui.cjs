/* Headless UI regression checks using the installed Edge runtime. */

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
    if (message.type() === 'error') {
      consoleErrors.push({ text: message.text(), location: message.location() });
    }
  });
  page.on('response', response => {
    if (response.status() >= 400) {
      failedResponses.push({ status: response.status(), url: response.url() });
    }
  });
  await page.addInitScript(() => {
    localStorage.setItem('render_source', 'blender');
    Object.defineProperty(window, '__RAVICHARA_DESKTOP__', { value: true });
    window.__ravicharaIpcMessages = [];
    window.ipc = {
      postMessage(command) {
        window.__ravicharaIpcMessages.push(command);
      }
    };
  });
  await page.goto('http://127.0.0.1:8760', { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => {
    const input = document.querySelector('#llmApiKey');
    return input && input.placeholder.includes('已保存在本地配置');
  }, null, { timeout: 15_000 });

  const wrapper = await page.locator('#huiWindowWrapper').boundingBox();
  const viewport = await page.locator('.hui-inner-content-viewport').boundingBox();
  if (!wrapper || !viewport || wrapper.width < 1365 || wrapper.height < 767 || viewport.width < 1350) {
    throw new Error(`window did not fill the WebView: ${JSON.stringify({ wrapper, viewport })}`);
  }

  await page.locator('.maximize-btn').click();
  const ipcMessages = await page.evaluate(() => window.__ravicharaIpcMessages.slice());
  if (ipcMessages.at(-1) !== 'maximize') {
    throw new Error(`maximize did not reach desktop IPC: ${JSON.stringify(ipcMessages)}`);
  }
  await page.setViewportSize({ width: 1600, height: 900 });
  await page.waitForTimeout(100);
  const expandedWrapper = await page.locator('#huiWindowWrapper').boundingBox();
  const expandedViewport = await page.locator('.hui-inner-content-viewport').boundingBox();
  if (
    !expandedWrapper || !expandedViewport
    || expandedWrapper.width < 1599 || expandedWrapper.height < 899
    || expandedViewport.width <= viewport.width || expandedViewport.height <= viewport.height
  ) {
    throw new Error(`core UI did not expand with the native window: ${JSON.stringify({ expandedWrapper, expandedViewport })}`);
  }
  await page.setViewportSize({ width: 1366, height: 768 });
  await page.waitForTimeout(100);

  await page.locator('#compactAvatarBadge').click();
  const frame = await page.evaluate(async () => {
    const response = await fetch('/api/blender/preview/status');
    const payload = await response.json();
    return {
      width: payload.stream.resolved_size[0],
      height: payload.stream.resolved_size[1]
    };
  });
  await page.waitForTimeout(250);
  const pane = await page.locator('#renderingWindowPane').boundingBox();
  const expectedWidth = pane.height * frame.width / frame.height;
  if (Math.abs(pane.width - expectedWidth) > 3) {
    throw new Error(`render pane aspect mismatch: ${JSON.stringify({ frame, pane, expectedWidth })}`);
  }
  const videoStatus = await page.evaluate(async () => {
    const response = await fetch('/api/video/status');
    return response.json();
  });
  const videoOptionCount = await page.locator(
    '#renderSourceSelectWrapper .custom-option[data-value="video"]'
  ).count();
  if (
    videoOptionCount !== 1
    || videoStatus.disk_cache !== false
    || videoStatus.chat_critical_path !== false
  ) {
    throw new Error(`external video adapter is incomplete: ${JSON.stringify({ videoOptionCount, videoStatus })}`);
  }
  await page.locator('#renderSourceSelectWrapper').evaluate(element => {
    element.dispatchEvent(new CustomEvent('custom-select-change', {
      detail: { value: 'video' }
    }));
  });
  await page.waitForTimeout(100);
  const unconfiguredVideoStatus = await page.locator('#renderStageStatusText').textContent();
  if (!unconfiguredVideoStatus.includes('未启用')) {
    throw new Error(`disabled video source did not fail safely: ${unconfiguredVideoStatus}`);
  }
  await page.evaluate(() => {
    document.querySelector('#videoEnabled').checked = true;
    document.querySelector('#videoSourceUrl').value =
      'http://127.0.0.1:8760/favicon.svg';
    document.querySelector('#videoSourceTypeWrapper').dataset.value = 'mjpeg';
    document.querySelector('#testVideoSourceBtn').click();
  });
  await page.waitForFunction(() => {
    const status = document.querySelector('#renderStageStatusText')?.textContent || '';
    return status.includes('外部视频') && status.includes('MJPEG');
  }, null, { timeout: 5_000 });
  const loadedVideoStatus = await page.locator('#renderStageStatusText').textContent();
  const externalFrameVisible = await page.locator('#blenderFrameImg').evaluate(element => (
    !element.hidden && element.classList.contains('is-visible') && element.naturalWidth > 0
  ));
  if (!externalFrameVisible) {
    throw new Error(`external MJPEG/image frame was not rendered: ${loadedVideoStatus}`);
  }
  await page.locator('#renderSourceSelectWrapper').evaluate(element => {
    element.dispatchEvent(new CustomEvent('custom-select-change', {
      detail: { value: 'blender' }
    }));
  });
  await page.waitForFunction(() => {
    const frameElement = document.querySelector('#blenderFrameImg');
    const status = document.querySelector('#renderStageStatusText')?.textContent || '';
    return frameElement?.src.startsWith('data:image/') && status.includes('Blender 实时');
  }, null, { timeout: 10_000 });
  const blenderRestoredAfterVideo = await page.locator('#blenderFrameImg').evaluate(
    element => element.src.startsWith('data:image/') && !element.hidden
  );

  await page.locator('#threeDotsBtn').click();
  const keyPlaceholder = await page.locator('#llmApiKey').getAttribute('placeholder') || '';
  if (!keyPlaceholder.includes('已保存在本地配置')) {
    throw new Error(`persisted key state is not visible: ${keyPlaceholder}`);
  }
  await page.locator('.tab-btn[data-tab="tabAppearance"]').click();
  await page.locator('.theme-option-card[data-bg="warm-beige"]').click();
  const controlColor = await page.locator('.maximize-btn').evaluate(
    element => getComputedStyle(element).color
  );
  if (controlColor !== 'rgb(15, 23, 42)') {
    throw new Error(`window controls did not adapt to light background: ${controlColor}`);
  }

  const screenshot = path.join(artifactDir, 'ui-regression.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  if (pageErrors.length || consoleErrors.length || failedResponses.length) {
    throw new Error(`browser errors detected: ${JSON.stringify({ pageErrors, consoleErrors, failedResponses })}`);
  }
  console.log('RAVICHARA_UI_TEST=' + JSON.stringify({
    wrapper,
    viewport,
    expandedWrapper,
    expandedViewport,
    ipcMessages,
    frame,
    renderPane: pane,
    videoStatus,
    unconfiguredVideoStatus,
    loadedVideoStatus,
    externalFrameVisible,
    blenderRestoredAfterVideo,
    apiKeyPlaceholder: keyPlaceholder,
    lightBackgroundControlColor: controlColor,
    screenshot
  }));
  await browser.close();
  browser = null;
})().catch(error => {
  console.error(error.stack || error);
  if (browser) browser.close().catch(() => {});
  process.exitCode = 1;
});
