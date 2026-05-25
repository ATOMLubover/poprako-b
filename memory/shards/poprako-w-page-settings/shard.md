---
name: poprako-w-page-settings
description: poprako-w 的设置页，负责切换汉化组、上传头像和退出登录
tags:
  - 设置
  - 头像
  - 账号
  - 退出
---

这是 `/settings` 页面。

## 页面定位

轻量设置页，目前主要做三件事：

1. 切换汉化组。
2. 上传头像。
3. 退出登录。

## 切换汉化组

切换后会直接影响：

1. `poprako-w-page-workspace`
2. `poprako-w-page-comic-playground`
3. `poprako-w-page-member-list`

## 上传头像

头像上传使用预签名上传流程。上传期间：

1. 会显示进度。
2. 会阻止用户轻易关闭弹窗。
3. 离开页面时会有保护提示。

## 退出登录

退出时会清空本地令牌与登录态，并跳回 `poprako-w-page-login` 对应的 `/login`。

## 不要误说

1. 不要说这里支持完整资料编辑。
2. 不要说这里支持修改密码或昵称；当前主要是切团队、传头像、退出登录。
