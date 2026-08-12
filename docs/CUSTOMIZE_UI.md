# UI customization

RaViChara 前端由 `static/index.html`、`static/theme.css`、`static/app.js` 和少量 SVG 组成。它们通过 Rust `include_bytes!` 嵌入 EXE，因此修改源码后必须重新编译；直接修改发布目录不会改变已编译的页面。

## 默认配色

米白主题的基础变量位于 `static/theme.css`：

```css
:root {
  --main-scene-backdrop: #fef3c7;
  --desktop-loop-bg: #fef3c7;
  --render-backdrop-bg: #bdbbf7;
  --accent-color: #9ff3fe;
  --accent-color-rgb: 159, 243, 254;
  --user-bubble-bg: #b6ebec;
  --character-bubble-bg: #ffffff;
}
```

设置页可以覆盖主题、主背景、渲染框、两类气泡和强调色。主题选择保存在 WebView2 `localStorage`；当前背景图与自定义立绘通过后端写入：

```text
data/ui_assets/background.image
data/ui_assets/avatar.image
```

图片不会以 base64 塞入 `localStorage`，响应头使用 `no-store`。项目不设置任意的 1.5 MB 上限；实际可用大小取决于磁盘、WebView2 解码能力和显存。高分辨率图片仍会增加启动解码时间与内存占用，不能据此推断为视频帧缓存。

## 角色头像

角色卡的 `avatar:` 可以指向 `characters/` 内的 PNG、JPEG、WebP、GIF 或 SVG。路径必须是相对路径，且后端会在规范化后确认文件仍位于 `characters/`。没有头像或文件读取失败时，界面显示内置矢量回退图。

用户在外观设置中上传的自定义立绘优先于角色卡头像。点击重置后，后端删除 `data/ui_assets/avatar.image`，界面回到当前角色卡头像；切换角色不会覆盖仍有效的用户自定义立绘。

角色卡格式见 [CHARACTER_CARDS.md](CHARACTER_CARDS.md)。

## 背景与外框

自定义背景只作用于主场景和外层背景图层。应用会采样背景的平均颜色，用于光晕和窗口按钮黑白对比，但不会把角色立绘颜色同步到外框。删除背景后恢复当前主题的纯色或预设背景。

右上角按钮由 Wry IPC 映射到原生最小化、最大化和关闭事件。最大化改变原生窗口和核心 WebView 尺寸，而不是只扩大透明边缘。

## 渲染窗

渲染窗背景使用 `--render-backdrop-bg`。展开时，前端根据 `/api/blender/preview/status` 返回的实际宽高比计算面板宽度；相机模式保持 Blender 相机比例，自定义模式使用设置中的明确尺寸。Blender、2D 和外部视频共用该显示区域，但帧源与缓存策略相互独立。

## 修改与验证

修改前端后至少运行：

```powershell
node --check static/app.js
cargo test --locked --offline
```

安装 Playwright 后，可在一个隔离数据目录和本地服务上运行 `scripts/verify_ui_offline.cjs`。该检查验证公开 Lily 资源、五项默认颜色、头像加载、窗口按钮对比度，以及未配置 Blender 时的安全降级。`scripts/verify_ui.cjs` 还包含需要真实 Blender 预览服务的完整渲染窗回归。
