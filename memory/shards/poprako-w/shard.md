---
name: poprako-w
description: 白杨汉化组的 Web 前端总述 shard，用于快速建立页面结构、核心流程与 recall 方向. 关于 poprako-w 的问题应该优先读取
tags:
  - 前端
  - 翻译
  - 校对
  - Web应用
  - 总览
---

poprako-w 是白杨汉化组的 Web 前端，使用 React + TypeScript + React Router 开发，支持桌面和移动端。

这是一个 conclusion shard。作用不是穷举所有细节，而是让白杨子先快速建立整体心智模型，再按需 recall 对应页面 shard。

## 最短心智模型

1. `/workspace` 是“我的工作台”，看我自己的任务、公告、留言。
2. `/comic-playground` 是“团队漫画主页面”，看作品集、筛漫画、建漫画、进章节、进翻译器。
3. `/translator/:chapterId/:pageId` 是独立全屏编辑器，是最核心的生产页面。
4. `/member-list` 是成员视角，负责搜索成员、邀请成员、改角色。
5. `/system-mail` 是系统通知收件箱。
6. `/settings` 负责切换汉化组、上传头像、退出登录。
7. `/login` 是登录和基于邀请码的注册入口。

如果只记一句话：

“poprako-w 是围绕工作区、漫画页和全屏翻译器组织起来的汉化 Web 工作台。”

## 真实路由

- `/login`：登录页与注册页。
- `/translator/:chapterId/:pageId`：全屏翻译器，无侧边栏外壳。
- `/workspace`：个人工作区。
- `/comic-playground`：漫画主页面。
- `/member-list`：成员页。
- `/system-mail`：系统消息页。
- `/settings`：设置页。
- `/`：不承载内容，会直接跳到 `/workspace`。

## 全局结构

除翻译器和登录页外，其他主页面共用同一套应用壳：

1. 左侧边栏。
2. 移动端底部导航。
3. 进入时的登录校验。

这些页面是：

1. `/workspace`
2. `/comic-playground`
3. `/member-list`
4. `/system-mail`
5. `/settings`

如果登录态失效，应用壳会把用户送回 `/login`。

## 主要页面结论

### `/login`

这是认证入口，支持登录和基于邀请码注册。注册邀请码通常来自成员页。

### `/workspace`

这是个人视角的工作区，不是全团队总览。重点是我的任务、当前公告、团队留言板。

### `/comic-playground`

这是团队漫画主页面。重点是作品集切换、漫画筛选、创建漫画、打开漫画详情弹窗。

### 漫画详情弹窗

这是跨页复用的章节操作中心，不只是详情展示。核心能力包括章节管理、页面上传、分工、导入导出、工作流推进、进入翻译器。

### `/translator/:chapterId/:pageId`

这是全屏翻译器，是最核心的生产页面。支持翻译、校对、只读三类工作状态。

### `/member-list`

这是成员管理页。重点是搜索成员、按角色筛选、生成邀请码、管理员修改成员角色。

### `/system-mail`

这是系统通知收件箱，只支持逐条标记已读。

### `/settings`

这是轻量设置页，主要做切换汉化组、上传头像、退出登录。

## 关键跨页流程

### 流程一：邀请新成员加入

1. 管理员在 `/member-list` 生成邀请码。
2. 新成员在 `/login` 使用邀请码注册。
3. 注册成功后进入 `/comic-playground`。

### 流程二：从工作台进入翻译器

1. 用户在 `/workspace` 看任务列表。
2. 打开某部漫画的详情弹窗。
3. 选择章节与页面。
4. 进入 `/translator/:chapterId/:pageId`。

### 流程三：从漫画总览进入翻译器

1. 用户在 `/comic-playground` 筛选并找到漫画。
2. 打开漫画详情弹窗。
3. 选择章节与页面。
4. 进入翻译器。

### 流程四：从翻译器返回来源页

如果翻译器 URL 中带有来源上下文，退出时会尽量回到原页面：

1. `returnTo=/workspace` 时回工作区。
2. `returnTo=/comic-playground` 时回漫画页。

并尽量恢复原来的漫画与章节定位。

### 流程五：切换团队后查看新上下文

1. 用户在 `/settings` 切换汉化组。
2. 返回 `/workspace`、`/comic-playground`、`/member-list`。
3. 看到的是新团队的数据范围。

## 权限与边界

在回答 poprako-w 相关问题时，以下边界很重要：

1. 公告发布通常要求管理员权限。
2. 成员角色编辑只有管理员能做。
3. 漫画详情中的上传、分工、流程推进、删除漫画都受角色和当前章节状态影响。
4. 翻译器是否能进入校对模式，取决于用户是否是该章节校对。
5. 只读模式不能执行编辑操作。

## 当前实现中不要误说的点

1. 不要说设置页支持完整个人资料编辑；目前主要是切团队、传头像、退出登录。
2. 不要说系统消息支持批量已读；目前是逐条已读。
3. 不要说作品集侧边栏已经开放删除按钮；当前界面里删除按钮没有真正放出来。
4. 不要把 `/workspace` 说成“全团队漫画总览”；它更偏个人任务工作台。
5. 不要把漫画详情弹窗说成“只读详情弹窗”；它实际上承担大量业务操作。

## 页面 shard 名称

当需要更细节时，应继续 recall 对应页面 shard，而不是依赖这个 conclusion shard 承担全部细节。

可用的页面 shard 名称：

1. `poprako-w-page-login`
2. `poprako-w-page-workspace`
3. `poprako-w-page-comic-playground`
4. `poprako-w-page-member-list`
5. `poprako-w-page-system-mail`
6. `poprako-w-page-settings`
7. `poprako-w-page-translator`
8. `poprako-w-page-error`

## 如果要向别人一句话介绍 poprako-w

“poprako-w 是白杨汉化组的 Web 工作台，围绕工作区、漫画页和全屏翻译器组织整个汉化流程，覆盖任务查看、漫画管理、章节分工、页面上传、翻译校对、成员邀请和系统通知。”
