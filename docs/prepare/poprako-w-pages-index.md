# Poprako W 页面手册索引

本文档组面向白杨子使用，按 `poprako-w` 的实际页面拆分说明每个页面的职责、核心功能、典型操作流程、限制条件与跨页跳转关系。

## 页面列表

1. `01-login-page.md`
   登录页与注册页手册。
2. `02-workspace-page.md`
   工作区页面手册。
3. `03-comic-playground-page.md`
   漫画总览与作品集页面手册。
4. `04-member-list-page.md`
   成员列表页面手册。
5. `05-system-mail-page.md`
   系统消息页面手册。
6. `06-settings-page.md`
   设置页面手册。
7. `07-translator-page.md`
   翻译器页面手册。
8. `08-error-page.md`
   错误页手册。

## 页面总览

1. `/login`
   用户登录、注册、获取访问令牌的入口。
2. `/workspace`
   当前用户的个人工作区，聚合任务、公告、留言板。
3. `/comic-playground`
   以作品集和漫画为核心的主操作页，负责筛选、创建、查看、推进流程。
4. `/member-list`
   以团队成员为核心的成员管理页，负责查询、邀请、角色调整。
5. `/system-mail`
   系统通知收件箱。
6. `/settings`
   团队切换、头像上传、退出登录。
7. `/translator/:chapterId/:pageId`
   全屏翻译/校对/只读编辑器。
8. `ErrorPage`
   路由异常或页面不存在时的兜底页。

## 关系说明

1. `/` 不承载内容，会直接跳转到 `/workspace`。
2. `/workspace`、`/comic-playground`、`/member-list`、`/system-mail`、`/settings` 共用同一套应用壳：左侧边栏、移动端底部导航、登录校验。
3. `/translator/:chapterId/:pageId` 是独立全屏页，但同样会在进入时检查登录态。
4. 漫画详情弹窗既会从 `/workspace` 打开，也会从 `/comic-playground` 打开；进入翻译器后，退出时会尽量返回原页面。
