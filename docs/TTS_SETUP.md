# TTS 配置与协议

## 可用模式

| Provider | 执行位置 | API key | 协议 |
|---|---|---:|---|
| `none` | 无 | 否 | 关闭 |
| `browser` | 浏览器 | 否 | `window.speechSynthesis` |
| `openai` | Rust 后端 | 是 | `POST /v1/audio/speech` |
| `mimo` | Rust 后端 | 是 | `POST /v1/chat/completions`，读取 Base64 音频 |
| `custom-openai` | Rust 后端 | 可选 | OpenAI-compatible `/audio/speech` |

配置和试听位于 UI 的“声音与朗读”选项卡。API key 保存在本机项目目录的
`data/overrides.json`，接口不会把明文密钥返回前端；便携包构建时会主动清除密钥字段。

## 后端接口

```text
GET  /api/tts/providers
GET  /api/tts/status
POST /api/tts/synthesize
```

合成请求：

```json
{"text":"你好，这是一段测试。"}
```

后端返回包含 MIME 类型和 `data:` URL 的 JSON。文本限制为 4096 个字符，音频响应限制为 16 MiB。

每条角色消息右下角都有重新朗读按钮。浏览器 TTS 每次创建新的
`SpeechSynthesisUtterance`；后端 TTS 每次重新调用供应商。项目不写入音频
文件，也不保留历史音频缓存；正在播放的 `Audio` 在结束、报错或切换消息后
立即释放。

## 小米 MiMo

默认参数：

```yaml
provider: mimo
base_url: https://api.xiaomimimo.com/v1
model: mimo-v2.5-tts
voice: 冰糖
response_format: wav
api_key_env: MIMO_API_KEY
```

MiMo TTS 的目标文本必须放在 `assistant` 消息中，发音风格作为可选 `user` 消息传入；这与 OpenAI `/audio/speech` 的请求体不同，因此后端使用独立适配器。

当前没有实现音色克隆文件上传。`mimo-v2.5-tts-voicedesign` 和 `mimo-v2.5-tts-voiceclone` 可以通过自定义模型名扩展，但相应的音频样本和专用参数仍需新增受限上传接口。
