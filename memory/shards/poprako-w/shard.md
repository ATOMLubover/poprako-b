---
name: poprako-w
description: 白杨汉化组的 Web 翻校前端（React/TypeScript），提供翻译编辑器、漫画浏览、工作区面板、成员管理、系统消息、设置等功能
tags:
  - poprako-w
  - 前端
  - 翻译
  - 校对
  - Web
  - React
---

poprako-w 是白杨汉化组的 Web 前端，使用 React + TypeScript + React Router 开发，支持桌面和移动端。以下是其完整功能范畴：

【页面路由】
- `/login`：登录页面
- `/translator/:chapterId/:pageId`：在线翻译编辑器（独立无侧栏布局）
- `/workspace`：工作区主页（侧栏布局）
- `/comic-playground`：漫画浏览与管理
- `/member-list`：团队成员一览
- `/system-mail`：系统消息信箱
- `/settings`：用户设置

【翻译编辑器（TranslatorPage）】
- 逐页在线翻译编辑：选择章节下的页面，在图片上直接编辑翻译单元（气泡文本框）
- 核心组件 `BaseTranslator`：提供翻译单元列表 + 图片叠加编辑界面
- 支持翻译/校对双模式操作
- 支持移动端和桌面端布局自适应

【工作区（Workspace）】
- 任务概览面板：查看当前用户被分配的翻译、校对、嵌字等任务
- 进度追踪：查看各漫画/章节的完成进度

【漫画浏览与管理（ComicPlayground）】
- 漫画列表：按作品集分组展示所有漫画
- 漫画详情弹窗：查看漫画的标题、作者、简介、封面、状态和章节列表
- 章节进度指示器：每个章节的工作流阶段进度条
- 作品集侧栏：切换不同作品集
- 嵌入式的漫画列表组件（可复用于其他页面）

【成员管理（MemberList）】
- 成员列表 + 筛选过滤（按角色、昵称等）
- 成员详情弹窗：查看成员信息
- 成员邀请弹窗：向新成员发送团队邀请
- 嵌入式成员列表组件

【系统消息（SystemMail）】
- 查看未读系统通知（工作流变更提醒、邀请通知等）

【设置（Settings）】
- 用户个人设置
- 头像上传
- 团队切换（同一用户可加入多个汉化组）
- 用户资料更新

【通用 UI 组件】
- Button、ConfirmDialog、IconInputRow、LoadingCircle、MultiProgressBar（进度条）、Paginator（分页器）
- HoverSelect 悬浮选择器
- NotificationToast 通知提示
- ToolboxDropdown 工具箱下拉菜单

【API 对接】
- 完整对接 poprako-s 后端的 RESTful API
- 覆盖所有核心资源：team、member、invitation、workset、comic、chapter、page、unit、assignment、assignmentInvitation、announcement、comment、sysMail、user、auth
