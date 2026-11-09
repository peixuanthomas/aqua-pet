# 设置界面字体

`fusion-pixel-12px-proportional-zh_hans-subset.ttf` 是
[Fusion Pixel Font](https://github.com/TakWolf/fusion-pixel-font) 2026.10.25
版 12px 比例简中字体的子集，只保留设置界面用到的字符，用于贴近
《Deltarune》菜单的点阵字形。字体以 SIL Open Font License 1.1 授权，
许可证见 `OFL.txt` 与 `LICENSES/`。

子集包含：

```
设置主动走跳散步距离随音乐舞动返回开关保存已档失败
0123456789% ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz:/·-
```

新增设置文字时需要用相同字体重新生成子集，否则 GDI 会回退到其它字体：

```powershell
pyftsubset fusion-pixel-12px-proportional-zh_hans.ttf `
  --text="设置主动走跳散步距离随音乐舞动返回开关保存已档失败0123456789% ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz:/·-" `
  --output-file=fusion-pixel-12px-proportional-zh_hans-subset.ttf `
  --name-IDs='*' --name-legacy --name-languages='*' --glyph-names `
  --notdef-glyph --notdef-outline --recommended-glyphs `
  --layout-features='*' --drop-tables+=DSIG
```
