# 规范化边界：parse / Display / Ord / Hash / matches

本文档由可执行回归测试支撑：

- `tests/test_normalization.rs`：表驱动语料，覆盖 parse 成败、Display 文本、
  二次 parse 的结构、排序关系、以及对固定 probe 集合的匹配向量。
- `tests/test_serde_normalization.rs`（仅 `serde` feature 下编译）：人类可读
  格式（JSON）与手写长度前缀二进制格式下的同样契约。

核心原则：**字符串往返、结构相等、匹配向量是三件不同的事**，不能用一个
`assert_eq!` 互相替代。

- 字符串往返：`parse(s)` 成功后，`to_string()` 得到什么文本？该文本再 parse
  是否得到同一个结构？
- 结构相等：`Eq` / `Hash` 比较哪些“身份字段”？全序 `Ord` 又多看了什么？
- 匹配向量：对一组 probe versions，`matches` 返回什么？向量相同只是对这组
  probe 的**有限见证**，不代表全域等价。

## 各类型的身份字段与被忽略字段

| 类型 | parse 入口 | 身份字段（参与 `Eq`/`Hash`） | `Ord` 额外/差异 | Display 用到 | matches 忽略 |
| --- | --- | --- | --- | --- | --- |
| `Version` | `Version::parse` | `major, minor, patch, pre, build` | 同身份字段；另有 `cmp_precedence` 只比 `major.minor.patch+pre`，忽略 `build` | 全部字段 | `build` 不参与；另受 prerelease 门槛约束 |
| `VersionReq` | `VersionReq::parse` | 有序的 `comparators: Vec<Comparator>` | **没有实现 `Ord`**（只有 `Eq/Hash/Debug`） | 空列表显示为 `*`，否则逗号空格连接 | build 已在 parse 时丢弃；prerelease 有额外门槛 |
| `Comparator` | `Comparator::parse` | `op, major, minor: Option, patch: Option, pre` | 无独立 `Ord`（derive 中无 Ord） | 缺省位不补全；`Op::Wildcard` 才追加 `.*` | build 在结构中根本不存在 |
| `Prerelease` | `Prerelease::new` | 内部 identifier 文本（`as_str`） | 空 prerelease 最大（正式版优先）；点分段：纯数字按数值、其余按 ASCII、数字段恒小于非数字段 | 原文 | `Comparator.matches` 按段比较；req 级另有兼容门槛 |
| `BuildMetadata` | `BuildMetadata::new` | 内部 identifier 文本 | 允许前导零：纯数字段先按去零数值、再按原始长度平局；空 build 最小 | 原文 | **从不参与** `matches` 与 `cmp_precedence` |

要点：

- `Version` 的 `Eq/Hash` **包含 build**，因此 `1.0.0+x != 1.0.0+y`；但
  `cmp_precedence`（SemVer “precedence”）与一切 `matches` 判断都忽略 build。
- `VersionReq` 没有全序；不要尝试对 req 排序。结构相等意味着 comparator
  序列（含 op、缺省位的 `None`、prerelease）逐项相同。
- `Comparator` 不保存 build，也没有字段能区分 `=1.0.0+a` 与 `=1.0.0+b`。

## Display 规范化（哪些拼写会变形）

| 输入拼写 | parse 后 Display | 说明 |
| --- | --- | --- |
| `1.2.3`（作为 VersionReq） | `^1.2.3` | 缺省操作符补成 caret |
| `1.0` / `1` | `^1.0` / `^1` | 缺省 minor/patch 保持 `None`，Display **不**补零 |
| `1.x` / `1.X` / `1.*.*` | `1.*` | wildcard 归并为 `Op::Wildcard`，缺省位规范化 |
| `1.2.x` | `1.2.*` | 同上 |
| `*` / `x` / `X` | `*` | 空 comparator 列表（`VersionReq::STAR`） |
| `=1.*` / `=1.*.*` | `=1` | **先前未在文档中点名的归并**：显式 op 的 wildcard 缺省位在结构里是 `None`，Display 不追加 `.*` |
| `>=1.2.3+meta` | `>=1.2.3` | comparator 上的 build 被接受但丢弃 |
| `^1.2.3-alpha+build` | `^1.2.3-alpha` | build 丢弃，prerelease 保留 |
| `  >=  1.0.0 , < 2.0.0 ` | `>=1.0.0, <2.0.0` | 仅 ASCII 空格在 req 起始、op 后、逗号前后、结尾被修剪 |
| 制表符 / 换行 | 报错 | 只有 `' '` 被当作空白；`\t`、`\n` 触发非法字符或“缺逗号” |

对**所有 parse 成功的输入**，`Display` 文本再次 parse 都得到结构相同的值
（`Version` 对合法输入甚至逐字符往返，`VersionReq`/`Comparator` 则先经过上述
规范化）。这个往返性质由表驱动测试逐行锁定。

`Version` 自身几乎不规范化：前导零只在 build 中合法且原样保留（`1.0.0+007`
仍显示 `+007`）；含字母的 prerelease 段允许以 0 开头（`-0a` 合法），纯数字
prerelease 段的前导零（`-01`）非法。

## parse 接受集边界（不改动，仅回归锁定）

- 数字段（major/minor/patch、纯数字 prerelease 段）禁止前导零；范围
  `0..=u64::MAX`，溢出报 `... exceeds u64::MAX`。
- `1.0.0-`、`1.0.0+`、`a..b` 报空段；非法 ASCII（`_`、`@`、NUL 等）报
  “unexpected character ...”，注意位置可能是“after <区段>”（解析在非法字节
  处停下），例如 `1.0.0-alpha_1` 报 “after pre-release identifier”。
- `Version` 不允许任何空白（包括首尾）；`VersionReq` 只接受 ASCII 空格且
  位置受限，空字符串与纯空格串都报错（不会被当成 `*`）。
- `Comparator::parse` 只接受**单个** comparator，`1.2.3, 2.0.0` 报错。
- wildcard 不能与其它 comparator 并列（`*, 1.0.0` 报错），也不能出现在
  具体数字之后（`*.1`、`1.*.1` 报错）。
- req 的 comparator 数量上限 32。

## Ord：数值 vs 字母、build 的前导零

- prerelease：`pre.8 < pre.12`（数值），但 `pre12 < pre8`（ASCII，因为
  `'1' < '8'`）；纯数字段恒小于含字母/连字符段；字段更多的更大；正式版
  （空 prerelease）最大。
- build：允许前导零并有独立全序，数值段平局时按**原始长度**比较，即
  `0 < 00 < 1 < 01 < 001 < 2 < 10`；空 build 最小，因此
  `1.0.0 < 1.0.0+1`（这是 `Ord`，不是 precedence）。
- 跨 build 的“优先级相等但结构不等”用 `cmp_precedence` 表达，不能用
  `assert_eq!` 代替。

## matches：Cargo 先行版本门槛（反直觉契约）

- 任何 req 要匹配 prerelease 版本，必须**至少有一个 comparator** 显式给出
  与被匹配版本相同的 major.minor.patch 且带非空 prerelease。
- 因此 `*`（STAR，空 comparator 列表）不匹配任何 prerelease，即使它匹配
  所有正式版；`*` 与 `>=0.0.0` 在本 crate 语义下全域一致，但二者结构不同
  （文档另有说明），测试中只以“有限见证”记录向量相同。
- req 级判断是“全部 comparator 数值成立 **且** 存在一个 prerelease 兼容
  comparator”；单独评估某个 comparator（`Comparator::matches`）只看它自己。
  反例：`Comparator::parse("<1.0.0")` 不匹配 `1.0.0-beta`，但 req
  `>1.0.0-alpha, <1.0.0` 匹配它（第一个 comparator 提供兼容门槛）。

### 零主版本 caret

- `^0.2.3`：锁定 minor==2，允许 patch 提升；
- `^0.0.3`：连 patch 都锁定（首个非零位是 patch）；
- 缺省位：`^0` 允许任意 minor，`^0.0` 锁定 minor==0、允许 patch。

### 比较器缺省位的边界

- `>1.0` ≡ `>=1.1.0`；`>1` ≡ `>=2.0.0`；`<1` ≡ `<1.0.0`；
  `=1.0` 允许 patch 提升而 `=1.0.0` 精确锁定。
- 缺省位在结构里是 `None`，即使两个 req 的 probe 向量相同，`None` 与
  `Some(0)` 仍是不同结构（例：`^1.0.0` vs `1.0.*` 在 1.1.0 处分叉）。

## 有限见证，而非全域等价

`tests/test_normalization.rs` 中 `finite_witness_*` 对每对“字符串不同、
结构不同、在给定 probe 上向量相同”的 req，都额外给出区分 probe：

- `^1.0.0` 与 `1.0.*`：`1.1.0` 匹配前者不匹配后者；
- `*` 与 `>=0.0.0`：正式版向量一致且都拒绝 prerelease（该一致性由 Cargo
  规则全域保证，文档承载等价性，测试只作见证）；
- `>=1.0.0-alpha` 与 `*`：在 `1.0.0-alpha` 处分叉。

新增 probe 时应同步更新向量；向量的长度必须等于 `PROBES` 长度（测试内置
断言）。

## serde 契约（feature = "serde"）

- `Version` / `VersionReq` / `Comparator` 序列化为其 `Display` 字符串；
  反序列化走同一个 parser，因此**不会**扩大或收紧接受集，非法字符串原样
  失败，类型不匹配（如 JSON 数字）也失败。
- `Prerelease` 与 `BuildMetadata` 没有独立 serde 实现，随 `Version` 表示。
- 人类可读（JSON）与非人类可读（测试内手写的长度前缀二进制）输出的字符串
  内容一致；这只是测试设施，crate 本身不引入任何二进制线上格式。
- serde 是纯增量 feature：默认构建完全不编译这些路径。

## 复杂度

- parse：`O(n)` 单次扫描；identifier 使用短字符串优化（≤8 字节内联），
  更长时单次堆分配；`VersionReq` 对 comparator 列表只做一次精确容量分配，
  comparator 上限 32。
- Display：`Version` 的长度计算与写入均为 `O(n)`，无分配（写进已有
  formatter）；`VersionReq` 为各 comparator 长度之和的线性开销。
- `Ord`（prerelease/build）：按点分段做线性比较；数字段按位数/数值比较，
  总体 `O(n)`。
- `matches`：每个 comparator `O(1)` 字段比较，整 req 为 `O(k)`（k 为
  comparator 数）；prerelease 门槛再线性扫描一次 comparator，仍为 `O(k)`。
- Hash：对规范化后的文本与数值字段做一次标准哈希，`O(n)`。

## 兼容性取舍

- 不改变任何 parse 接受集与错误文案；新增测试只回归当前行为。
- 未记录但已存在的行为（`=1.*` 显示为 `=1`、仅 ASCII 空格被修剪、
  req/comparator 丢弃 build、build 的前导零全序、req 无 `Ord`）以测试与
  本文档固定，避免下游误依赖或误假设。
- 不把任何逻辑移到外部服务；serde 测试额外用到的 `serde_json` 仅为
  dev-dependency，版本已由现有依赖树提供。
