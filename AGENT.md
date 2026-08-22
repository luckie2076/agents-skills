1. 代码极简, 优先官方库或第三方成熟库, 而非自己造轮子
2. 应该基于最新的库或包 (大版本), 最新的文档
3. 基于官方的使用教程或者业界最佳实践
4. 对于不确定的情况, 请尽可能让用户选择, 而不是自作主张
5. 使用英文 (代码/注释/git commit), 但文档和回复使用中文
6. 发布新版本 cargo 包: 一律通过 GitHub Actions, 流程为 bump Cargo.toml 版本 → 提交并推送 → 打 tag `vX.Y.Z` 并推送 (release.yml 监听 `v*` tag, 自动执行 `cargo publish`), 不要本地执行 `cargo publish`

