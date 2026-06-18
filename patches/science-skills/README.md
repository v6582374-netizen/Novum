# science-skills 本地补丁记录

当前没有对 `vendor/science-skills` 源码做本地 patch。

约束：

- 不直接修改上游源码来适配 Novum UI。
- 需要行为差异时，在 `services/research` adapter 层处理。
- 如果未来必须修改上游文件，需要在本目录记录 patch 文件、原因、影响范围和回滚方式。
