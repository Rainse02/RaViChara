# Blender 接入与测试

## 当前实现

Rust 后端已经实现 Blender TCP 客户端协议：

- 4 字节大端长度前缀 + UTF-8 JSON；
- `version`、`request_id`、`token`；
- `system.ping`；
- `scene.inspect`、`mmd.list_models`、`expression.inspect`；
- Virtual_c：`expression.apply {model, expression, intensity, replace}`；
- Virtual_c：`rig.create_action {armature, preset}`，随后调用 `rig.play`；
- 连接、读写超时和响应帧大小限制。

项目内另有一个 Blender 伴生扩展：

```text
blender_addons/ravichara_preview_bridge/
```

它挂接到 Virtual_c 已有的 dispatcher，不另开端口，增加：

- `render.preview.status`；
- `render.preview {width, height, refresh, transparent}`。
- `render.viewport {width, height, transparent}`。
- `ravichara.avatar.capabilities {model, armature}`；
- `ravichara.behavior.play_plan {keyframes, morphs, ...}`；
- `ravichara.behavior.execute {generation, idle, behavior}`。

v0.3.0 使用 `GPUOffScreen` 直接读取当前 3D View 的 RGBA 帧缓冲并在内存中
编码 PNG，不依赖 `Render Result`，也不创建临时帧文件。
`render.viewport` 优先使用活动相机，并在捕获期间临时关闭网格和骨骼等视口
叠加层；没有活动相机时才使用当前 3D View。该命令供无队列 WebSocket 帧流使用；
`render.preview` 保留当前渲染引擎，供单帧高质量检查使用。
Blender 4.5 的帧缓冲通过显式 RGBA `UBYTE` 读取，避免 GPU 纹理的打包布局被
误当成连续像素后形成规则白色网格。
状态接口同时返回活动相机的渲染分辨率；后端可在配置的宽高上限内保持相机比例，
也可使用自定义传输尺寸。

动作使用一次性生命周期：播放范围包住动作范围，前段从当前姿态过渡到动作，
后段返回由人物卡性格参数与模型现有骨骼共同生成的轻量待机循环。伴生扩展只复用
一个临时 Action，不会让每条消息持续增加 Blender Action 数据块。表情在保持时间
结束后回到 neutral。

v0.5.5 不再以十个预设动作作为主要控制方式。RigProfile v3 会读取 mmd_tools 元数据、
当前 IK/FK 约束与开关、固定轴、附加变换和锁定通道，再报告语义骨骼映射、过滤后的
精确骨骼名、精确形态键以及逐轴旋转上限；后端把该快照、人物卡摘要和本轮
对话交给隔离的行为规划器。规划器返回相对于待机姿态的 0–1 归一化关键帧，Rust 与
Blender 两侧分别校验目标、数量、时序和角度。临时 Action 播放完即移除，形态键恢复
原值并回到人物卡待机。鞠躬、点头等预设仅供 2D、旧插件或计划无效时降级。材质、
shader、持久姿势和正式 Action 仍必须经 UI 确认；材质修改前会复制原材质。

当 IK 完全启用时只移动目标控制骨并屏蔽对应 FK 链；IK 关闭时才开放受限 FK 旋转。
部分 influence 混合、重叠约束或无法确定的控制链会按 fail-closed 策略禁用该肢体，
不会让 LLM 同时驱动 IK 与 FK 导致双重变换。

目前只支持：

| 模式 | 行为 |
|---|---|
| `off` | 不连接 Blender |
| `pose` | 根据对话和当前模型能力生成临时表情/关键帧计划，失败时使用安全预设降级 |

`snapshot`、`animation` 和 `auto` 尚未接入当前 Rust/UI 数据流，因此配置校验不会接受这些值。

## 实际联调条件

安装 Blender 本身并不足以测试。还需要兼容插件在 Blender 进程内启动 TCP 服务。当前默认配置与 Virtual_c 插件一致：

```yaml
blender:
  enabled: true
  profile: virtual_c
  host: 127.0.0.1
  port: 9876
  token: ""
  token_env: RAVICHARA_BLENDER_TOKEN
  model_name: ""
  render_mode: pose
```

如插件启用了 token，应在配置或 `RAVICHARA_BLENDER_TOKEN` 中设置相同值。通用请求结构为：

```json
{
  "version": 1,
  "request_id": "...",
  "command": "system.ping",
  "params": {},
  "token": "..."
}
```

响应应为：

```json
{"ok": true, "data": {...}}
```

或：

```json
{"ok": false, "error": {"code": "...", "message": "..."}}
```

## 检查

应用启动后访问：

```text
GET http://127.0.0.1:8760/api/blender/status
```

执行完整协议诊断：

```http
POST http://127.0.0.1:8760/api/blender/test
Content-Type: application/json

{"inspect":true,"expression":"happy","motion":"nod"}
```

检查画面扩展并取得 UI 可直接显示的 PNG：

```text
GET http://127.0.0.1:8760/api/blender/preview/status
GET http://127.0.0.1:8760/api/blender/frame?refresh=true&width=512&height=512
WS  ws://127.0.0.1:8760/api/blender/stream?fps=12&width=512&height=512
```

结果会明确区分：

- integration disabled；
- 端口无法连接或超时；
- 插件响应 `system.ping`；
- 场景、模型和表情检查；
- 表情与动作各自的成功或错误。

`virtual_c` 档位会在未配置 `model_name` 时调用 `mmd.list_models`。只有一个模型时自动选择其模型和骨架；多个模型时要求显式配置，避免误操作。表情会先检查当前模型的 `preset_support`，对语义别名选择可用的安全预设。

离线测试验证帧格式、token、响应解析、模型自动发现和 Virtual_c 原生参数。本机真实联调已验证 Blender 4.5.11 LTS、Virtual_c 插件、单模型场景、`happy` 表情别名和 `nod` 动作。测试表情和动作会修改 Blender 当前内存场景，但应用不会主动保存 `.blend` 文件。

## 安装画面回传扩展

1. 通过 Blender 的“从磁盘安装”选择
   `blender_addons/ravichara_preview_bridge-0.5.5.zip`。
2. 先启用 Virtual_c，再启用 **RaViChara Preview Bridge**。
3. 在 3D View 的 `RaViChara` 侧栏确认显示 `Attached to Virtual_c`。
4. 回到应用的 Blender 选项卡，将渲染源切换为 Blender。画面自动连接；
   “重连实时画面”只用于恢复断开的 WebSocket。

扩展只允许 128–1024 px PNG。实时流没有帧队列：下一帧只会在上一帧完成并发出后再捕获，因此慢渲染不会持续堆积内存。扩展会恢复分辨率和透明背景设置，不保存 `.blend`，也不保存图片缓存。

如果扩展缺失、渲染失败或超时，UI 保留 2D 角色和状态错误，不会把占位图误报为 Blender 输出。

## LLM MCP 动作

RaViChara 同时提供本机 Streamable HTTP MCP：

```text
POST http://127.0.0.1:8760/mcp
```

普通对话工具包括 `ravichara_get_avatar_capabilities`、
`ravichara_apply_behavior_plan` 和兼容用的 `ravichara_apply_reaction`。
模型不需要等待用户输入控制命令：它先读取当前角色能力，再根据对话生成一次性计划。
涉及材质、shader、持久骨骼姿势或 Action 的工具只生成十分钟内存提案，必须由 UI
明确确认后才执行。MCP 调用同时广播 UI 实时事件，因此 2D 可显示近似的降级动作。

LM Studio 不允许 `ephemeral_mcp` 连接环回地址。要让通过 1234 端口发起的
模型请求调用本机 MCP，需在 LM Studio 的 `mcp.json` 中加入：

```json
{
  "mcpServers": {
    "ravichara": {
      "url": "http://127.0.0.1:8760/mcp"
    }
  }
}
```

随后在 LM Studio 中启用经过身份验证的 “Allow calling servers from
mcp.json”，并设置：

```yaml
llm:
  mcp_integration_id: mcp/ravichara
```

未完成这项授权时，`mcp_mode` 为 `local_server_only`。普通聊天仍会在可见回复完成后
通过后端的隔离结构化规划请求生成相同行为计划，不依赖控制标签，也不会因为 LM
Studio 的私网 MCP 限制返回 502。DeepSeek、MiMo 和其他 OpenAI-compatible 服务同样
使用这条后端规划路径。
