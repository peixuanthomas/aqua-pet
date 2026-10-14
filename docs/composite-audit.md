# AQUA 组合动作核对记录

本记录对应 `tools/build_composite_actions.py` 生成的 25 段独立动作、2,269 个时间采样、90,860 个绘制图层，使用其中 25 组原始素材。完整的 94 组、346 张图片由资源页另行覆盖；资源组数量不等于动作数量。

`verified: true` 表示这段片段的图片选择、帧顺序、公式、参数和图层顺序已与原脚本核对。它不表示已录制原游戏并逐像素比较。所有片段都保留脚本文件名及行号；这里核对的是固定参数下的独立动作，不是完整的 Deltarune 战斗模拟器。

## 来源与重建方式

- 原文件：`C:/Users/STRIX/Documents/Codex/2026-09-30/ssh/work/chapter5-data.win`。
- 原文件 SHA-256：`370dfd141d2955d5a1960122919b16e4092b52ffbb85fda541bc4680c6b3b85c`。
- 脚本来自同目录 `ch5-all/CodeEntries` 的完整 GML 导出；对象默认 sprite 和两段 shader 另用 UndertaleModCli 的只读 `load --line` 核对，没有保存或改写游戏。
- `obj_attack_knifechain_manager2` 默认 sprite 是 `spr_enemy_aqua_chainswing_body`；`obj_attack_knifefan_manager` 默认 sprite 是 `spr_enemy_aqua_idle`。
- `obj_plat_enm_aqua_miniboss`、`obj_plat_enm_aqua_sword` 默认 sprite 是 `spr_faqua`；small、gold、bouncer 分别是 `spr_faqua_small`、`spr_faqua_gold`、`spr_faqua_spin`。
- 数字资源 2517/7424 分别是 `spr_omegaf_aqua` / `spr_omegaf_aqua_o`；3614 是 `spr_bullet_knife_break`。
- 时间单位固定为 1/30 秒。生成器先按原代码计算原作场景坐标，再把坐标、移动及绘制倍率除以 2，存为原始图片像素单位。角度保留 GameMaker 的逆时针角度；屏幕向下为正 y。
- 所有随机输入均在代码中固定，方便每次重播比较；使用 Python 的固定种子抽取原作允许范围内的参数，并没有声称复刻 GameMaker 的随机数序列。
- 动作片段以其入口为 t0。需要场景目标、战斗框或玩家位置的片段，在动作说明中给出固定值。没有把未知的对话等待时长编造成循环。

## 已做的片段

| 片段 | 核对的行为 | 主要原作依据 |
| --- | --- | --- |
| 双刀链，6 把 / 8 把 | 同时存在两个 manager；后臂→身体→前臂；35 tick 挥动间隔；12 tick 预备；20 tick 正弦缓出手臂；刀链跟随、残影及回收旋转 | `obj_attack_knifechain_manager2_Create:25,122`、`Draw:7`、`Step_2:98`、`Alarm_0:54`、`obj_dbulletcontroller_Step:3403` |
| 原作测试双刀链 | 独立 arm 图层随刀角旋转；后臂半透明；170 tick 后回收；首把刀按原作隐藏 | `obj_enemy_aqua_example_Draw:127,201`、`obj_attack_knifechain_manager_Step:39` |
| 扇形飞刀角色 | 准备图层在身体后；索引4→6用4 tick，0→2用2 tick；32 tick稳定循环 | `obj_attack_knifefan_manager_Draw:1`、`Step:5,35` |
| 扇形飞刀，3 / 4 把 | 抛出15 tick、回收预备5 tick，然后瞄准固定目标；加速；标准战斗框内灰色裁切副本 | `obj_attack_knifefan_manager_Create:30`、`Step:22`、`obj_bullet_knife_Alarm_0:5` |
| 花瓣刀环，6 / 12 瓣 | 每 tick 出现一瓣；同一时刻启动；灰色蓄力、缩放、旋转散开；先画所有后半片，再画所有前半片 | `obj_attack_knife_leafling_Create:9,80`、`Draw:5`、`obj_aquabullet_Step:26` |
| 花形 AQUA 悬停 | 5 帧，每帧4 tick | `obj_plat_enm_aqua_miniboss_Create:44` |
| 小花漂浮 | x 横幅16、频率.15；y 纵幅6、频率.4；5帧每帧4 tick | `obj_plat_enm_aqua_small_Create:30`、`Step:23` |
| 左 / 右闪避 | 10 tick 二次缓出，横向12、向上4原作像素 | `obj_plat_enm_aqua_miniboss_Step:53,54` |
| 弹性压缩 | 绘制前根据 squishtimer 独立修改横纵缩放，绘制后恢复 | `obj_plat_enm_aqua_bouncer_Draw_76:4`、`Draw_77:2` |
| 金色花悬浮 | 满血时 image_speed .25；横幅30、纵幅6；固定 phase_offset 0 | `obj_plat_enm_aqua_gold_Create:33`、`Step:17,88` |
| 受击旋转形态 | bouncer 受击换为 `faqua_blur`，继承 image_speed .25 | `obj_plat_enm_aqua_bouncer_Create:7,61` |
| 模糊旋转跃向同伴 | 12 tick，jumpspeed18，fakegravity3，落点最后一帧对齐 | `obj_dw_cliff_seth_miniboss_Step:693,697`、`obj_jump_to_point_Other_10:6,147` |
| 过场顶部 / 左侧 / 右侧刀阵 | 7 / 5 / 5把刀，原作1倍刀尺寸，4 tick淡入，8 tick横向back缓出及纵向二次缓出 | `obj_dw_cliff_sethaqua_battle_Create:31,64,97`、`ease_out_back:3` |
| Omega 幽灵 | 轮廓、本体、全息加色三遍绘制；10 tick进入；固定时间与抖动样本 | `obj_plat_finalflowery_Draw:31`、`Create:182,218` |
| 持剑冲刺 | 独立剑角度；蓄力、冲刺、挥剑拖影；青色加亮残影 | `obj_plat_enm_aqua_sword_Step:47,86`、`Draw:4` |
| 格挡后失剑 | 从停顿结束后的 con3 开始；角色后退；刀向玩家反方向飞、水平镜像、每tick转50°；完整60 tick寿命 | `obj_plat_enm_aqua_sword_Create:129,155`、`Step:135` |
| 三连投刀和换位 | hp9、单敌人、未保护；t41/46/51投刀，刀出现时转540°；t93开始15 tick换位 | `obj_plat_enm_aqua_miniboss_Step:21,138,175`、`Create:23` |
| 立体转向刀 | 准备、淡入、转向目标；帧0→6→0，缩放、灰转白、减速和加速 | `obj_plat_aquaknife_3d_Create:18`、`Step:45,59` |
| Omega 刀阵 | 6×6布局、-3°/tick、速度4、60 tick改一次方向；边缘透明、±300回绕、减速、最后淡出 | `obj_dbulletcontroller_Step:3269,3291`、`obj_omega_knife_Step:2,17` |

上表省略统一的 `gml_Object_` / `gml_GlobalScript_` 前缀和 `_0.gml` 后缀；生成的 JSON 保存完整、可定位的文件名。

## 容易误还原的地方

- `chainswing_front/back` 的资源默认速度是16 FPS，但正式链条动作使用 `back_index/front_index` 和 `scr_lerpvar` 驱动，不能按16 FPS一直播放。起始索引是4.5；第一次 Alarm 加1，再以20 tick推进5帧。
- 正式链条两种手臂位置是 `(17,60)`，身体位置是 `(32,75)`。把三张图按左上角重叠会错位。图像自身原点也必须保留。
- 正式链条残影来自上一帧的 `xprevious/yprevious`；残影初始宽2、高1、透明度.35，每tick分别减2/15、1/15、.35/15。不是在当前刀位置再叠一张。
- 花瓣刀是两个分开的图片层，但原作先画**全部**第0帧，再画**全部**第1帧。逐把画完整刀会改变相交处的遮挡。
- 扇形飞刀灰影是142×142透明画布的裁切结果，不是给全部刀直接变灰。标准150×150战斗框且中心(-190,70)时，原始像素单位裁切为 `[-130,0,-59,71]`。
- 失剑方向是从玩家指向角色。目标在右方时，刀向左飞、水平镜像，旋转是+50°；摩擦要减整个速度向量的长度，不只减水平速度。
- `scr_jump_to_point` 给新建的跳跃身体复制目标 marker 的速度。随后设置 NPC 的 `.5` 并不改变先前新建 marker 的默认速度1。
- `plat_aquaknife_3d` 原脚本把 `contimer == 30` 的翻转写在 `contimer < 30` 分支里，永远不会执行。重建保留了这一原作行为，没有擅自“修复”。
- `chainswing_back_old` 和 `knifespin_temp` 不用仅凭名字拼成新动作；原始图片仍可在资源页预览。

## 颜色与桌面绘制

灰色蓄力使用 `tint` 逐通道乘色。青色残影使用 `solid` 把有图案的位置画成同一种颜色，再以 `blend:"add"` 加亮；原作每tick降低透明度.04，.3初值最多持续8 tick。

`shd_grayscale_fade` 的灰度公式为 `0.21R+0.72G+0.07B`，输出是 `原色×fade+灰度×(1-fade)`，fade=hp/maxhp。这里的满血花形片段 fade=1，保留原色。

`shd_chromatic_reflection` 用屏幕坐标计算彩光。预览固定原作640×480画面的中心为(320,240)，把片段坐标还原为原作2倍尺寸，再计算条纹和色相；时间从0毫秒开始。输出保留原shader的3.7亮度、.5强度和加色。它使用屏幕位置，不使用 PNG 的贴图坐标。

Debug 在所选背景上逐层画，可直接核对加色效果。Windows 透明桌宠最终用普通透明度混合到桌面，无法读取并给其他程序的像素加亮；这是与原游戏背景上的加色效果仍有差别的具体位置。

## 验证与剩余范围

生成器自检所有资源、帧号、数值、颜色范围，并检查：准备/投刀索引、刀链关键手臂帧、收招时刻、花瓣两遍绘制顺序、首瓣延迟、灰转白时刻、闪避最终位置、480 tick刀阵时长以及幽灵三图层。重复运行得到相同 JSON。

没有声称完成的内容：

1. 没有原游戏逐帧录像比对。运动按原脚本和选定时间起点采样，不运行 GameMaker 的整套对象调度；这个限制尤其影响多个计时对象在同一tick创建或销毁时的首帧对齐证明。
2. 玩家碰撞、伤害、地图墙壁/地面、整场战斗调度以及对话触发均不在本阶段。反弹类角色的局部压缩和形态已做，地图上的完整反弹路线需要碰撞输入。
3. 金色花周围的 `spr_bellsparkle`、刀飞出战斗框时的 `spr_falling_petal`、受击烟雾和音效不属于本次94组AQUA素材，不加入独立本体动作。失剑片段从格挡停顿结束后开始，未伪造一个省略停顿的完整交战。
4. 加色桌面限制如上。普通原色、透明边缘、位置、缩放、角度和图层顺序不受此限制。

重建命令：

```powershell
python tools/build_composite_actions.py --code C:/Users/STRIX/Documents/Codex/2026-09-30/ssh/work/ch5-all/CodeEntries
```

该命令只读脚本与 `assets/sprites.json`，只写 `assets/composite-actions.json`。
