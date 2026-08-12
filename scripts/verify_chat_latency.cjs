/* Live first-turn streaming latency probe for a running RaViChara server. */

const endpoint = process.argv[2] || 'ws://127.0.0.1:8760/api/chat/stream';
const message = process.argv[3] || '只回复 RAVICHARA_LATENCY_OK';
const socket = new WebSocket(endpoint);
const timeoutMs = 190_000;
let sentAt;
let firstDeltaMs = null;
let deltaChars = 0;
let done = false;
const eventCounts = {};

const timer = setTimeout(() => {
  if (!done) {
    console.error(`chat stream timed out after ${timeoutMs} ms`);
    socket.close();
    process.exitCode = 1;
  }
}, timeoutMs);

socket.addEventListener('open', () => {
  sentAt = performance.now();
  socket.send(JSON.stringify({
    message,
    client_timestamp: new Date().toISOString(),
    elapsed_minutes: 0
  }));
});

socket.addEventListener('message', event => {
  const value = JSON.parse(String(event.data));
  eventCounts[value.type] = (eventCounts[value.type] || 0) + 1;
  const payload = value.data || {};
  if (value.type === 'delta') {
    if (firstDeltaMs === null) firstDeltaMs = Math.round(performance.now() - sentAt);
    deltaChars += String(payload.content || '').length;
    return;
  }
  if (value.type === 'error') {
    done = true;
    clearTimeout(timer);
    console.error(JSON.stringify({ type: 'error', data: payload }));
    socket.close();
    process.exitCode = 1;
    return;
  }
  if (value.type !== 'done') return;

  done = true;
  clearTimeout(timer);
  const result = payload.result || {};
  if (firstDeltaMs === null || !result.reply) {
    console.error('stream completed without a visible delta or reply');
    socket.close();
    process.exitCode = 1;
    return;
  }
  console.log('RAVICHARA_CHAT_LATENCY_TEST=' + JSON.stringify({
    first_delta_ms: firstDeltaMs,
    total_ms: Math.round(performance.now() - sentAt),
    delta_chars: deltaChars,
    reply_chars: String(result.reply).length,
    provider: result.provider,
    model: result.model,
    usage: result.usage,
    prompt_metrics: result.prompt_metrics,
    event_counts: eventCounts,
    avatar_control_path: result.avatar_control_path,
    time_sent_every_turn: result.time_sent_every_turn
  }));
  socket.close();
});

socket.addEventListener('error', () => {
  if (done) return;
  done = true;
  clearTimeout(timer);
  console.error('chat WebSocket connection failed');
  process.exitCode = 1;
});
