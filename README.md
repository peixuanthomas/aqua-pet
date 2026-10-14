# AQUA Rust 桌宠

Windows 10/11 x64 桌宠。图片、动作数据和运行库都包含在 `dist/AQUA.exe` 中。运行时不需要 Rust、Codex、游戏文件、Steam Deck 或网络。

## 使用

双击 `dist/AQUA.exe`。角色透明、无边框、始终置顶；首次出现在主屏工作区右下角，原始像素放大 4 倍。

- 左键拖动角色；透明区域可点击下面的窗口。
- 右键角色，选择 **设置**、**Debug · 动作预览** 或 **关闭**。
- 默认蓝色晃动为完整 8 帧，每帧 200 毫秒，一轮 1.6 秒。
- 自动走动直接复用上下左右四组原作走动动画，每次随机选择一个方向走 1～3 秒，然后在当前位置恢复晃动；靠近屏幕边缘时提前停下。
- **设置 → 随机走动概率**：拖动滑条立即生效。最左侧 0% 完全不走动，最右侧 100% 经常走动，默认 25%。每轮 1.6 秒晃动结束后，按这个概率决定是否出发；它不是走动时间占比，也不改变行走速度。拖动角色和打开 Debug 时暂停自动走动。
- **设置 → 走动距离倾向**：左侧更容易选到短路线，右侧更容易选到长路线，中间为长短均衡（默认）。两端也保持随机，不会变成固定距离。每次出发时重新选择 1～3 秒的路程，默认 4 倍约 240～720 屏幕像素；靠近屏幕边缘时可能更短。调整从下次走动生效，不改变正在走的路线或行走速度。
- 行走速度采用原作资料室场景：每秒 60 个素材像素，默认 4 倍时每秒 240 个屏幕像素；动画每 4 次游戏更新换帧，约 133 毫秒/帧。显示倍率改变时，移动距离等比例改变。
- 位置、倍率、走动概率和距离倾向保存在 `%LOCALAPPDATA%\AquaPet\settings.json`，重启后保留。旧版设置文件仍可读取，缺少距离倾向时默认长短均衡。删除这个文件可以恢复初始设置。
- 启动时直接打开面板：`AQUA.exe --debug`。
- 已有桌宠运行时，再次执行 `AQUA.exe --debug` 会打开它现有的面板，不增加一个桌宠。

## Debug 面板

左侧用分类和搜索查找动作或原始资源名。共有 **225 个条目：131 个原作片段 + 94 组原始素材**，包含完整 **346 张 PNG**。

选择条目后，桌宠和预览使用同一个播放状态。可以暂停、重播、单次/循环、上一帧/下一帧、拖动时间条、调整 0.25～4 倍播放速度，以及恢复原速。复杂动作的“帧”是原作一次更新的画面，通常为 1/30 秒；状态栏同时显示采样号和图片帧号。多图层动作的图片帧号指最后一个可见图层。

“桌宠大小”支持 1～8 倍整数像素。动作范围太大时，会降低实际倍率以放进当前屏幕，状态栏显示实际值。面板预览根据可用空间单独适配，可以切换深色、浅色、棋盘格背景。

动作带有原作位移时，桌宠会真实移动。单次结束回到起点，循环每轮从起点开始；拖动更新起点。靠近边缘时会调整起点以容纳整段动作。关掉 Debug 面板恢复默认晃动，右键“关闭”退出整个程序。

对白等待会显示“等待继续”，按“继续”推进；不把未知的玩家等待时间编成固定秒数。弹幕仅供查看，不实现碰撞、伤害或整场战斗。自动行为只包含默认晃动与随机走动，其他动作仍由 Debug 手动选择。

## 原作还原的范围

所有图片均从本机未修改第五章文件提取，保留原始透明画布、尺寸和对齐原点。Codex 旧版六帧适配素材保存在 `assets/codex-reference/`，没有覆盖当前 Codex 宠物。

131 个原作片段包括 106 个角色预设和 25 个组合/攻击预设。按原脚本分别保留场景速度、局部等待、位移、旋转、手臂与身体顺序、飞刀和残影；81 组资源被动作引用，其余资源仍完整收录在素材列表。未使用、头像、界面图片和旧零件不虚构成原作动作。

**“片段已对照原作脚本”表示其参数和绘制方式有脚本依据，不表示已完成原游戏录像的逐帧比对。** 独立片段采用固定的场景目标和随机样本，不复刻完整房间、其他角色、声音、战斗和触发条件。同一更新中多个游戏对象的创建顺序仍需原游戏运行画面进一步核对。

Debug 在指定背景上绘制加亮特效。Windows 透明桌宠只能与桌面进行普通透明度混合，不能真正把其他软件的像素加亮；因此全息和加亮残影在桌面上的颜色可能与游戏不同。

核对记录：

- [角色动作](docs/character-audit.md)
- [组合、攻击及特效](docs/composite-audit.md)
- [验证记录](docs/validation.md)
- [素材和引用脚本校验值](docs/integrity.json)

## 构建

需要 Rust MSVC 工具链和 Visual Studio C++ Build Tools。交付版本使用 Rust 1.98，edition 2024；仅使用 `windows`、`image`、`serde`、`serde_json`。

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

输出为 `target/release/aqua-pet.exe`。`.cargo/config.toml` 为 Windows x64 静态链接 C 运行库。`tools/package.ps1` 会执行上述检查，复制到 `dist/AQUA.exe` 并记录 SHA-256。

## 重新提取与核对

已经提取的素材随项目保存，普通编译无需执行下面的步骤。若更新游戏版本，应先重新核对脚本，不能直接沿用旧版动作数据。

```powershell
.\tools\extract_assets.ps1 `
  -GameData 'C:\path\chapter5-data.win' `
  -UndertaleModCli 'C:\path\UndertaleModCli.exe' `
  -SpriteTree 'C:\path\sprite-tree.json'

python tools/build_character_actions.py --code-dir C:/path/CodeEntries
python tools/build_composite_actions.py --code C:/path/CodeEntries
python tools/validate_assets.py --game-data C:/path/chapter5-data.win --code-dir C:/path/CodeEntries --report docs/integrity.json
```

提取工具只读游戏文件。`SpriteTree` 是原素材仓库 Git tree API 的递归 JSON；核对版本为 `46727ff283dcad40bb6c316694f1ee0eef53cdca`，目录为 `Characters/Bosses/Ch5/Aqua/`。Python 工具只使用标准库。UndertaleModCli 是独立的开源提取工具，需自行提供，未打包进桌宠。

原第五章 SHA-256：`370dfd141d2955d5a1960122919b16e4092b52ffbb85fda541bc4680c6b3b85c`。

Steam Deck 对应安装文件与本机修改版的 SHA-256 均为 `0bbbd30c4e7f225b667912c606c97247b9f498b9223ee3eae3a0c37f051b02c0`；此前比较的 632 个 AQUA 相关脚本无差异。动作依据采用未修改文件的导出脚本。

## 项目文件

| 文件 | 作用 |
|---|---|
| `src/catalog.rs` | 读取内置素材和动作，检查引用和帧号 |
| `src/player.rs` | 推进时间、暂停、单次/循环、逐帧及手动继续 |
| `src/render.rs` | 对齐图片，按层绘制旋转、缩放、透明度和特效 |
| `src/placement.rs` | 将完整动作放入当前屏幕工作区 |
| `src/wander.rs` | 随机选择方向、按原作速度移动、控制走动概率和停留 |
| `src/platform.rs` | Windows 桌宠窗口、设置滑条、原生 Debug 控件和设置保存 |
| `assets/*-actions.json` | 可审查的动作定义和原作依据 |
| `tools/export_aqua.csx` | 从游戏原始资源导出图片、原点和校验值 |

角色及游戏素材来自 DELTARUNE，属于原权利人。此项目保存用户已提供游戏中的素材，未包含或修改游戏本体。

参考：[原素材仓库](https://github.com/jasonvorheez1/deltaruneorganizedspritearchive)、[UpdateLayeredWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-updatelayeredwindow)、[SetCapture](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcapture)。
