# 存档边框

`save-frame.png` 用《DELTARUNE Chapter 5》的 `spr_textbox_topleft` 第 0 帧、`spr_textbox_top` 和 `spr_textbox_left` 拼成。游戏的 `scr_darkbox` 也用这三张图绘制边框。

两个黑底框沿用 `obj_savemenu` 的保存成功画面。上框居中显示「设置」，下框只留一个存档。绘制时按双倍像素大小显示，以保留边角的像素形状。

源游戏文件的 SHA-256 为 `370dfd141d2955d5a1960122919b16e4092b52ffbb85fda541bc4680c6b3b85c`。

# AQUA 台词气泡

`aqua-bubble-tail.png` 来自同一份游戏数据的 `spr_battleblcon_parts` 第 4 帧，裁掉了透明留白。原帧的绘制原点是 (36, 12)，裁剪左上角是 (20, 2)。

`aqua-bubble-dotumche.png` 是原作英文 `fnt_dotumche` 的字形图。`aqua-bubble-dotumche.json` 记录各字形在图上的位置、大小和左侧留白。

AQUA 使用 `scr_enemyblcon` 的第 10 种气泡。白底由两个矩形拼成，尖尾指向右侧。文字用 9 像素字距和 20 像素行距，一行最多 37 个字符，按每秒 30 个游戏帧的速度逐字显示。边框按整段文字预先定好大小，单行时尖尾高度减半。
