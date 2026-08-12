# Versioning and compatibility

RaViChara 分别维护桌面应用、Blender 扩展和桥接协议版本。三者描述的问题不同，不应为了让数字一致而同时递增。

## 桌面应用

桌面应用使用语义化版本 `MAJOR.MINOR.PATCH`，唯一来源是 `Cargo.toml`：

- `MAJOR`：配置、数据或公共接口发生不能自动迁移的不兼容变化；
- `MINOR`：向后兼容的新能力、较大的默认行为或界面调整；
- `PATCH`：不改变预期接口的修复、性能或稳定性改进。

`src-tauri/tauri.conf.json` 保留为项目元数据，`resources/windows.rc` 提供 EXE 文件属性中的 `FileVersion` 与 `ProductVersion`；三者必须与 `Cargo.toml` 相同。Git 标签使用 `v<应用版本>`，例如 `v0.6.0`。

## Blender 扩展

扩展独立使用语义化版本。版本来源是 `blender_addons/ravichara_preview_bridge/blender_manifest.toml`，并与扩展 `__init__.py` 报告值一致。

桌面应用在 `src/blender/mod.rs` 和 `static/app.js` 中声明最低兼容扩展版本。扩展增加可选命令时通常递增 `MINOR`；只修复实现而不改变命令结构时递增 `PATCH`；删除或重定义命令时递增 `MAJOR`，并评估桥接协议是否也需升级。

## 桥接协议

桥接协议使用单调递增整数。目前为 `4`。只有消息帧、认证、命令或响应结构发生不向后兼容的变化时才递增。新增带能力发现的可选字段不要求升级协议。

## 当前兼容矩阵

| 桌面应用 | 最低扩展 | 桥接协议 | 说明 |
|---:|---:|---:|---|
| `0.6.0` | `0.5.5` | `4` | RigProfile v3、PMX 语义手臂/IK、相机 PNG 传输边界和待机回归 |

## 发布顺序

1. 更新实现和测试；
2. 仅递增确实发生变化的组件版本；
3. 运行 `scripts/release_preflight.ps1`；
4. 运行完整测试和 Windows 打包；
5. 提交干净工作区；
6. 创建与应用版本一致的带注释标签；
7. 推送标签，由 GitHub Actions 创建 Release。

不得重复使用已经公开的标签或覆盖已发布二进制。若构建产物错误，应修复后递增应用 `PATCH` 并建立新 Release。
