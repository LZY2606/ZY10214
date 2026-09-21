# 规范化边界说明（由可执行测试支撑）

本文档说明本 crate 在 **parse / 存储 / Display / Ord / Hash / matches** 各条路径上，
五种类型各自的“身份字段”与“被忽略字段”，并给出对应回归测试的位置。所有结论都由
表驱动测试锁定，而不是仅靠文档断言。

- 测试：
  - `tests/test_normalize.rs`：Version / Comparator / VersionReq 的表驱动语料、
    build metadata 契约、有限见证与四类独立反例、比较器数量与空白边界。
  - `tests/test_identifier_contract.rs`：Prerelease / BuildMetadata 的语法接受集、
    排序链与前导零反例。
  - `tests/test_serde_normalize.rs`（仅 `serde` feature 下编译）：人类可读 JSON 与
    自包含二进制 wire format 的规范化与 feature 开关反例。
- 运行：`cargo test --locked --all-features`；不需要网络、时钟或文件系统遍历顺序。

## 三个必须分开的概念

1. **字符串往返（round-trip）**：`parse(text)` 成功后，`x.to_string()` 是否与 `text`
   逐字符相等。
2. **结构相等（structural identity）**：`a == b`、`Hash`、以及 `Ord` 使用的字段集合。
3. **matches 等价（predicate equivalence）**：两个 `VersionReq` / `Comparator` 对版本
   谓词的取值。

三者彼此独立。常见错误是用一个 `assert_eq!(a.to_string(), b.to_string())` 或一句
“在几个 probe 上结果相同”同时替代三者。测试里分别断言，不混用：

- 往返：`test_normalize.rs::version_table_parse_display_reparse`、`comparator_table`、
  `req_table`，每个样例都比较“输入 / Display 文本 / 再次 parse 的结构 / Display
  幂等性”四件事。
- 结构：`version_identity_fields`、`req_identity_is_ordered_comparator_sequence`、
  `prerelease_identity_and_empty_constant`、`test_identifier_contract.rs`。
- 谓词：统一 probe 版本库上的 0/1 匹配向量（`match_vector`），以及
  `finite_witness_same_vector_but_not_globally_equivalent`。

### 有限见证，而非全域等价

probe 向量相同**只**说明在那一组版本上不可区分。两个直接反例：

- `*` 与 `>=0.0.0-0`：对全部稳定版的向量一致，但在 `0.0.0-alpha` 上相反。裸 `*`
  不匹配任何 prerelease；后者的 comparator 带非空 prerelease 且 mmp 精确，故放行。
- `~1.2.3` 与 `=1.2.3`：在只含 `1.2.3` / `1.2.3-alpha` 的 probe 上一致，但 `1.2.4`
  立刻区分（tilde 放行 patch 升级，exact 不放行）。

反向地，全域谓词相同也不代表结构或字符串相同：`>=1.0.0, <2.0.0` 与交换顺序的
`<2.0.0, >=1.0.0` 谓词一致、结构不等；`^1` / `1.*` / `=1` / `~1` 的规范化文本各异，
谓词在当前实现下全域一致。这由 `same_predicate_does_not_mean_same_structure` 锁定。

## 每种类型的身份字段与被忽略字段

| 类型 | 结构身份字段（参与 `Eq`/`Hash`/`Ord`） | Display 依据 | matches 使用 | 被丢弃/忽略 |
| --- | --- | --- | --- | --- |
| `Version` | `major`,`minor`,`patch`,`pre`,`build` | 全部五字段 | `major`,`minor`,`patch`,`pre`（外加 prerelease gate） | matches 不看候选版本的 `build` |
| `VersionReq` | 有序的 `comparators: Vec<Comparator>`（顺序敏感） | 逐项 Comparator Display，以 `", "` 连接；空列表固定 `*` | 每个 comparator 都须满足 | 原字符串里的空白、`x/X` 拼写、缺省 caret 不保留；comparator 上的 build 在 parse 时丢弃 |
| `Comparator` | `op`,`major`,`minor`,`patch`,`pre` | 同五字段；`Wildcard` 在缺省位补 `.*` | 同五字段 | 没有 build 字段：`+meta` 解析后即丢 |
| `Prerelease` | 内部 identifier 字符串（按 dot 分段语义比较） | 原字符串（无前导零可保留，纯数字段前导零本就非法） | 逐段比较 + prerelease gate | — |
| `BuildMetadata` | 内部 identifier 字符串 | 原字符串（前导零原样保留） | **完全不参与** | 对 precedence/matches 不可见 |

要点：

- **build metadata 参与结构相等与显示，却不参与版本优先级。**
  `build_metadata_contract` 用四组独立断言锁定：(a) `Eq`/`Hash` 区分不同 build；
  (b) Display 保留 build（含合法前导零）；(c) `cmp_precedence` 对 build 视而不见；
  (d) `VersionReq::matches` 与 `Comparator::matches` 都不看候选版本 build，且要求侧
  书写的 build 在 parse 阶段就被丢弃。
- Version 的派生 `Ord` 把 build 当最后一级排序键；需要 SemVer “precedence” 时使用
  `Version::cmp_precedence`。
- Comparator 上的 build 与候选版本上的 build 是两件事：前者 parse 即丢，后者只在
  `Eq`/`Hash`/`Ord`/Display 中可见。

## 语料覆盖矩阵

`tests/test_normalize.rs` 对每个样例分别断言：parse 是否成功（失败则锁定错误文本）、
Display 文本、再次 parse 的结构、排序关系（仅 Version）、对统一 probe 库的匹配向量。
统一 probe 共 32 个（24 个稳定版 + 8 个 prerelease，最后一个 prerelease 带 build），
索引固定，便于直接书写期望向量；失败信息会按顺序打印 probe 列表。

| 边界 | 代表样例 |
| --- | --- |
| 前导零 | 数字位 `07.0.0`/`1.01.0`/`1.0.00` 拒绝；pre 纯数字段 `1.2.3-01` 拒绝；pre 含字母 `0a` 允许；build `+001`/`+00` 允许且 Display 保留 |
| 空 prerelease/build | `1.2.3-`、`1.2.3+`、`1.2.3-+x`、`1.2.3++`、`a..1` 拒绝；`Prerelease::new("")`/`BuildMetadata::new("")` 得到空常量 |
| ASCII 非法字符 | 下划线 `rc_1`、非 ASCII `é`、NUL、内部空白、多余点均拒绝，错误文本锁定 |
| 超大整数 | `u64::MAX` 三段均合法；`u64::MAX+1` 在 major/minor/patch 三个位置分别报 overflow；pre/build 里的 23 位数字串合法且按长度排序 |
| wildcard | `*`/`x`/`X` 为空 comparator 列表且 Display `*`；`1.*`/`1.x`/`1.*.*` 同构；`=1.*` 变 `=1`；`*.*`/`*.1`/`1.*.1`/`*, x` 拒绝 |
| 缺省 minor/patch | Version `1`、`1.2` 拒绝；Comparator/Req 接受，`1`→`^1`、`1.2`→`^1.2`；`<1.2` 对 `1.2.0-alpha` 的反例 |
| 多个 comparator | 逗号统一 `", "`；operator 两侧空白被吃；顺序是身份字段；最多 32 个，第 33 个报错；缺逗号 `>=1.0.0 <2.0.0` 拒绝 |
| 空白 | 只容忍 ASCII 空格（req 首尾、operator 与逗号两侧）；tab 与内部空格拒绝；Version 任何位置都拒绝空白 |
| serde 人类可读 | JSON 以规范化 Display 字符串编解码；非规范空格/拼写在往返后被改写；非法输入复用 parse 接受集 |
| serde 二进制 | `test_serde_normalize.rs` 内的极简 wire format（`is_human_readable()==false`）同样得到 Display 文本，反序列化走同一 parser |

## 四类独立反例（各自一个 self-contained 测试）

1. **prerelease 数值 vs 字母排序** — `counterexample_prerelease_numeric_vs_alpha`：
   点分隔纯数字段按数值（`pre.2 < pre.11`）；整段含字母按 ASCII（`pre11 < pre2`）；
   数字段恒小于字母段（`1 < 1a0 < a`）。
2. **零主版本 caret** — `counterexample_zero_major_caret`：`^1.2` 放行 `1.3.0`，
   `^0.2` 不放行 `0.3.0`；`^0.0` 再收紧到 `0.0.x`。
3. **比较器缺失位** — `counterexample_missing_components`：`=1` 覆盖整个 1.x 而非
   仅 `1.0.0`；`<1.2` / `>1.2` 在 minor 相等且 patch 缺失处立即决定，prerelease 行为
   与完整写法 `<1.2.0` 不同。
4. **serde feature 开关** — `tests/test_serde_normalize.rs` 整体 `#![cfg(feature =
   "serde")]`：默认 feature 下整文件不编译，`--all-features` 下执行 JSON、嵌套元组与
   二进制路径。

另有 `counterexample_wildcard_with_explicit_op`（`=1.*` 显示为 `=1`）与
`counterexample_hyphen_is_not_a_range`（`1.2.3 - 2.3.4` 拒绝；`1.2.3-2.3.4` 被解析成
prerelease，而非 npm 风格区间）记录拼写陷阱。

## 语义与复杂度取舍

- 比较结果是字段数值比较与 identifier 分段比较，**不做任何整数解析以外的数值计算**；
  prerelease/build 的数字段以“位数 + ASCII 文本”决定顺序，因此不溢出，长度只受内存
  限制。数字 major/minor/patch 仍是 `u64`，溢出是 parse 错误。
- prerelease gate（Cargo 规则）在 `eval.rs` 中实现：候选带非空 prerelease 时，要求中
  必须至少存在一个 comparator 拥有**完全相同的 major.minor.patch**与非空 prerelease，
  且每个 comparator 本身仍须满足。这解释了为什么 `*`、`>=1.2.3-alpha` 等对大多数
  prerelease 返回 false。
- `VersionReq` 以有序 `Vec<Comparator>` 存储，结构相等对顺序敏感；合取的交换律只体现
  在谓词层。匹配对 comparator 数量做线性扫描，上限 32 个。
- 规范化只发生在 Display，parse 不“修复”输入（不会把 `1.01.0` 当成 `1.1.0`），也不
  扩展接受集；任何被 Display 改写的拼写都能在本文件测试中找到对应样例。

## 兼容性

- 未修改任何 parse 接受集、错误文本或公开 API；仅新增测试与文档，并增加
  dev-dependencies（`serde_core` 化名 `serde`、`serde_json`），只影响测试构建。
- 支持既有平台与 feature 组合：默认（std）、`--no-default-features`（no-std）、
  `--all-features`（serde）。serde 测试在无 serde feature 时不编译。
- 测试不访问外网、不依赖真实时钟/等待，也不依赖文件系统遍历顺序。
