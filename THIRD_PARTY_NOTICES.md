# Third-party notices

本文件说明 RaViChara 所依赖或可连接、但不由项目 MIT License 自动覆盖的内容。它不是法律意见，也不会替代相应权利人的许可条款。

## 默认示例角色

`characters/lily.card.yaml` 与 `characters/lily.avatar.svg` 是为 RaViChara 编写的原创虚构示例，包含在仓库的 MIT License 中。它们不基于外部作品角色，也不需要在线下载素材。

用户本机可以在 `characters/` 中加入其他角色卡和图片。此类私有资源不因被 RaViChara 加载而获得 MIT 授权；公开提交或二进制分发前，使用者必须自行确认作者、来源、修改权和再分发权。仓库的默认忽略规则会排除当前维护者的私有甘雨卡与头像，但忽略规则不能代替对其他新增文件的权利审查。

## Blender 与扩展

`blender_addons/ravichara_preview_bridge/` 在 Blender 内通过 `bpy` API 运行。独立分发或与 Blender 组合分发时，应核对 Blender GPL 和 Blender 扩展平台的现行要求。Blender 本体不随本项目分发。

## Virtual_c 协议

桥接线协议参考 [Rainse02/Virtual_c_blender](https://github.com/Rainse02/Virtual_c_blender) 的公开接口行为重新实现。项目未声明该外部仓库代码的所有权，其许可证和素材仍按原仓库条款处理。

## 用户导入内容与外部服务

用户自行导入的 PMX/MMD 模型、角色卡、图片、声音、视频和其他素材仍受各自许可证约束。LLM、TTS、视频模型及云 API 也受供应商条款约束；RaViChara 的本地配置功能不构成对这些服务或内容的再授权。
