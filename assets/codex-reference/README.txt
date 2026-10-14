Aqua — Codex 原生切帧修复版

已修复：自动晃动、挥手只显示第一帧。
原因：原 APNG 上传并保存到云端后只剩静态首帧，而这些动作的各格内容相同。
修复：使用单张静态 PNG，每个格子放入对应原作姿势，由 Codex 逐格播放。
保留现有 v1 格式、原作像素、4 倍整数放大及鼠标悬停和拖动动作。

状态：
- idle / 空闲：6 格左右晃动，6.6 秒一轮。
- running / 工作：6 格左右晃动，0.82 秒一轮。
- waiting / 等待回答或批准：挥手。
- review / 结果未读：挥手。
- 其他动作：原作走路、转圈、受伤。

限制：Codex 固定空闲、工作各 6 格和各格停留时间，不能通过 pet.json 改成原作 8 帧、每帧 200 ms。
提醒动作依当前 Codex 播放规则，三遍后回到空闲。不能保证整个等待期间持续挥手。
完整原作 8 帧仅保留在 aqua-sway.gif 参考文件中；不要把它当成应用内实际速度。

本地目录：%USERPROFILE%\.codex\pets\aqua-deltarune
如果使用已同步到云端的 Aqua，仅替换本地文件不会更新云端副本，必须更新同一个云端宠物。

来源：https://github.com/jasonvorheez1/deltaruneorganizedspritearchive
素材树：46727ff283dcad40bb6c316694f1ee0eef53cdca
角色及原游戏美术：Toby Fox 与 DELTARUNE 原作者。
