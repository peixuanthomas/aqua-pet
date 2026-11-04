# AQUA-pet

<p align="center">
  <img src="docs/images/aqua-idle.gif" alt="AQUA 默认左右摇晃" width="152" height="128">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="docs/images/aqua-wave.gif" alt="AQUA 挥手" width="104" height="136">
</p>

将《Deltarune Chapter 5》中的人气角色 AQUA 做成桌宠。

> **开发状态**：项目仍处于早期开发阶段，功能尚不完善，目前仅支持 Windows 11（x86_64）。

## 如何使用

从源码构建需要 **Rust MSVC 工具链**和 **Visual Studio C++ Build Tools**。测试环境使用 Rust 1.98，项目采用 Rust 2024 edition。

克隆项目后，依次执行检查、测试和构建：

```powershell
git clone https://github.com/peixuanthomas/aqua-pet.git
cd aqua-pet
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

构建完成后，运行 `target/release/aqua-pet.exe`，即可显示无边框的桌宠角色。

在 Windows 桌面运行 `cargo test -- --ignored`，可以验证桌宠跟随窗口边沿、Debug 播放和暂停时的重画范围，以及设置和 Debug 面板的红心和悬停颜色。测试结束后会自动关闭临时窗口。

Windows 构建会自动将 `assets/sprites/spr_aqua_sleevewave_0.png` 转为多尺寸图标并写入 EXE，设置和 Debug 窗口的标题栏也使用该图标。图标保留透明背景和角色比例，无需额外携带图标文件。

- **在窗口上走路**　AQUA 把当前显示器工作区的底部当作地面。它会沿地面走近普通应用窗口，跳到露出的上沿，再左右散步或跳下。重叠窗口只留下能站稳的部分。拖动脚下的窗口时 AQUA 会跟随，边沿被遮住或窗口最小化后，它会落到下方的窗口或地面。
- **左键拖动**　抓起 AQUA 可以带它换位置或换屏，松手后自然下落。打开 Debug 可以接管动作和位置，关闭后恢复自动活动。
- **日常动作**　待机时 AQUA 偶尔挥袖、鼓掌或转圈。AQUA 站稳时，把鼠标移到它约一个身宽以内，它会挥一次手。再次挥手需要鼠标先离开、再靠近，两次至少间隔 15 秒。连续两分钟没有键鼠输入时，它会在落稳后坐下或露出无聊表情。有新输入就恢复活动。
- **右键菜单**：打开桌宠设置或 Debug 动作预览面板。
- **托盘图标**：运行期间在右下角通知区域显示 AQUA 图标，左键或右键点击均打开与角色右键相同的菜单，可进入设置、Debug 动作预览或关闭程序；退出后图标移除，任务栏重启后自动恢复。Windows 可能将图标收进“隐藏的图标”区域，可在任务栏设置中调整显示位置。
- **桌宠设置**　每轮待机 1.6 秒后，AQUA 按“主动走跳”设置的概率开始走路或跳跃。将概率调到 0% 会停止主动走跳，已经在空中的 AQUA 会继续落地。“散步距离”控制每段散步偏短还是偏长。按 Tab 切换滑块，用方向键或滚轮调节，修改自动保存。
- **Debug 动作预览**　在左侧搜索和筛选动作。右侧可以暂停、重播、逐帧播放或拖动时间轴，也可以调整播放速度、桌宠大小和预览背景。点击“动作信息”可查看原始资源、说明和原作依据。

在设置和 Debug 面板中，红心标记当前选中的项。打开下拉菜单时，红心移到当前选中的值。鼠标划过其他选项时，该项变黄，红心留在原处。

设置保存在 `%LOCALAPPDATA%\AquaPet\settings.json`。

启动时添加 `--debug` 参数（`aqua-pet.exe --debug`），可直接打开 Debug 动作预览面板。

## 项目还原

项目从游戏本体中提取了 **131 个原作动作片段**，包括 **106 个角色预设**和 **25 个组合／攻击预设**。

素材保留了原始的透明画布、尺寸和对齐原点，因此部分图片可能因原作中的位置设置而出现偏移。项目还通过反编译游戏程序，获取原作的动画播放速度和角色行走速度，尽量还原游戏中的表现。

## 著作权

本项目使用的游戏图片素材来自《Deltarune Chapter 5》，著作权归原权利人所有。项目仅使用游戏素材，**不包含游戏本体**。
