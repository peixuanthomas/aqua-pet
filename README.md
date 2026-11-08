# AQUA-pet

<p align="center">
  <img src="docs/images/aqua-idle.gif" alt="AQUA 默认左右摇晃" width="152" height="128">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="docs/images/aqua-wave.gif" alt="AQUA 挥手" width="104" height="136">
</p>

将《Deltarune Chapter 5》中的人气角色 AQUA 做成桌宠。

> **开发状态**：项目仍处于早期开发阶段，功能尚不完善，目前仅支持 Windows 11（x86_64）。

## 简介

AQUA 是无边框的桌面角色，会显示在普通应用窗口上方。它把屏幕底部和窗口上沿当作地面，会在上面走动、坐下或跳下，也可以用鼠标拖动到任意位置。

- 鼠标靠近时会挥手，待机时会随机挥袖、鼓掌或转圈。
- 播放音乐时，待机会换成舞步。
- 相邻窗口之间会短距离翻滚跳跃，翻滚结束时落到另一个窗口上沿。起落点在 1 倍大小时横向最多相差 50 像素、高低最多相差 35 像素，随角色大小同比增加；窗口与底部任务栏之间沿用原有跳跃动作。
- 右键点击角色或托盘图标，可以选择“坐下”、打开桌宠设置和 Debug 动作预览。“坐下”会打断当前动作或舞步，沿用 12 秒后起立的规则，再继续原来的动作判定；空中、拖动或 Debug 预览时不可用。

## 如何构建

从源码构建需要 **Rust MSVC 工具链**和 **Visual Studio C++ Build Tools**。

```powershell
git clone https://github.com/peixuanthomas/aqua-pet.git
cd aqua-pet
cargo build --release
```

构建完成后，运行 `target/release/aqua-pet.exe` 即可显示桌宠。

## 著作权

本项目使用的游戏图片素材来自《Deltarune Chapter 5》，著作权归原权利人所有。项目仅使用游戏素材，**不包含游戏本体**。
