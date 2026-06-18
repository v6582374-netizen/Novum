# science-skills 上游来源记录

- 上游仓库：https://github.com/google-deepmind/science-skills
- 上游 commit：33557e0f1faf0f281d255940de58935c61b2143b
- 导入日期：2026-06-18
- License：Apache License 2.0
- Novum 导入位置：`vendor/science-skills`
- Novum 用户可见入口：仅解析 `vendor/science-skills/skills/**/SKILL.md`

## 本地改动

当前没有修改上游源码。Novum 通过 `services/research` 中的 adapter 读取技能元数据，并隐藏上游 `scripts/`、`references/` 等原始目录结构。

## 同步升级

建议使用以下流程升级上游快照：

```sh
git ls-remote https://github.com/google-deepmind/science-skills.git HEAD
git clone --depth 1 https://github.com/google-deepmind/science-skills.git /private/tmp/novum-science-skills
rsync -a --delete --exclude .git /private/tmp/novum-science-skills/ vendor/science-skills/
```

升级后需要同步更新：

- 本文件中的上游 commit 和导入日期。
- `patches/science-skills/README.md` 中的本地 patch 说明。
- Phase 4 技能解析测试快照。
