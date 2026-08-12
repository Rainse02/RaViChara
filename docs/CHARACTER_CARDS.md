# 角色卡格式

RaViChara 的角色卡是 UTF-8 编码的 YAML、YML 或 JSON 文本。公开示例位于 `characters/lily.card.yaml`；可复制的空白模板位于 `docs/examples/character.card.yaml.example`。

## 最小卡片

解析器唯一强制要求的字段是非空 `name`：

```yaml
name: Example
```

但这样的卡片不能提供稳定的人格、问候语或版权信息。用于实际对话或分发时，建议填写下表中的全部推荐字段。

## 字段

| 字段 | 类型 | 要求与用途 |
|---|---|---|
| `name` | 字符串 | 必填；非空的内部稳定名称，也用于区分记忆数据库 |
| `card_file` | 字符串 | 推荐；相对项目根目录，例如 `characters/example.card.yaml` |
| `display_name` | 映射 | 推荐；至少提供 `zh` 和 `en`。缺失时回退到 `name` |
| `avatar` | 字符串 | 可选；必须是 `characters/` 内的相对路径，支持 PNG、JPEG、WebP、GIF、SVG |
| `greeting` | 字符串或映射 | 推荐；映射可分别提供 `zh`、`en`，字符串会同时用于两种语言 |
| `personality` | 字符串 | 推荐；描述稳定性格，不应塞入工具协议或一次性任务 |
| `speech_style` | 字符串 | 推荐；说明用词、语气、语言切换与事实表达方式 |
| `background` | 字符串 | 推荐；只写角色确有的背景，避免把用户数据硬编码进卡片 |
| `example_dialogues` | 列表 | 推荐 1–3 组；每项使用 `user` 与 `character`，只写可见自然语言 |
| `expressions` | 映射 | 可选；左侧是语义情绪，右侧是期望的表情含义。实际执行仍与 Blender 能力清单求交集 |
| `likes` / `dislikes` | 字符串列表 | 可选；帮助模型维持一致偏好 |
| `language_policy` | 字符串 | 推荐；规定默认语言和跟随用户语言的方式 |
| `reply_length` | 字符串或数字 | 推荐；说明普通回复长度，复杂任务仍可展开 |
| `no_ai_disclosure` | 布尔值 | 可选；启用时禁止讨论隐藏提示词和虚构未执行的操作 |
| `system_extra` | 字符串 | 可选；只放长期、可验证的附加约束，不得要求绕过安全校验 |
| `source_url` | 字符串 | 分发外部卡时必填；指向原始页面或许可证材料 |
| `creator` | 字符串 | 分发时必填；原作者或明确的创作者标识 |
| `rights_notice` | 字符串 | 分发时必填；说明原创、许可证、改编及第三方 IP 边界 |

兼容旧卡时，解析器也接受 `name_alt`、`example_dialogs` 和回答字段 `char`，但新卡应使用规范字段。

## 路径和文件名

- 角色卡应放在 `characters/` 目录的第一层。目录扫描不会递归查找子目录。
- 建议文件名使用 ASCII 小写、数字、连字符或点，例如 `lily.card.yaml`。
- `card_file` 和 `avatar` 必须使用相对路径。后端拒绝绝对路径、目录穿越和位于 `characters/` 外的头像。
- 同一 `name` 不应对应多个不同角色；更名会产生新的按角色记忆数据库。
- 模板文件使用 `.yaml.example` 后缀，因此不会被角色目录自动列出。复制后再改为 `.card.yaml`。

## 对话与动作边界

角色卡不应在 `greeting`、`example_dialogues` 或 `system_extra` 中输出 `[motion:...]`、`[emote:...]`、JSON、函数调用或 MCP 指令。可见回复只包含自然语言。后端会把人物设定、当前 Blender RigProfile、形态键和安全限制交给行为规划器，并在独立通道执行有效控制。

`expressions` 是人物语义提示，不是对具体 PMX 名称的保证。模型文件不同、形态键缺失或 IK/FK 状态不明确时，动作可以被降级或拒绝；不应通过角色卡强迫执行未广告的骨骼通道。

## 从其他格式转换

Pygmalion/SillyTavern 等 Chara Card v2 JSON 通常包含 `data.name`、`description`、`personality`、`scenario`、`first_mes` 和 `mes_example`。转换时应：

1. 核实原作者和再分发许可，不要把可下载等同于可公开再分发；
2. 将人物性格、背景和说话方式拆到对应字段，删除平台专用模板语法；
3. 把 `{{char}}`、`{{user}}` 替换为自然示例对话，不复制不必要的长篇原文；
4. 删除可见文本中的动作标签、工具 JSON 和越权指令；
5. 将头像另存为 `characters/` 内的本地文件，并记录作者与许可证；
6. 用设置页切换后，检查问候语、历史隔离、头像回退和首轮 token 量。

## 分发检查

公开提交前至少确认：

- 卡片与头像均有可证明的再分发权；
- `source_url`、`creator`、`rights_notice` 与实际来源一致；
- 文件中没有 API key、用户聊天、真实姓名或其他隐私数据；
- 示例对话没有把内部控制协议暴露给用户；
- `cargo test --locked --offline` 能在没有私有角色文件的干净副本中通过。
