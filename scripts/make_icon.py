#!/usr/bin/env python3
"""Gera o ícone do app (USB stick) em PNG 1024. cyrix: Pillow puro, sem assets externos.
Uso: python3 scripts/make_icon.py  → escreve scripts/rufus-icon.png
Depois: cargo tauri icon scripts/rufus-icon.png  (gera src-tauri/icons/*)
"""
from PIL import Image, ImageDraw, ImageFilter

S = 1024
cx = cy = S // 2

# --- fundo: rounded-rect com gradiente vertical ---
grad = Image.new("RGB", (1, S))
top, bot = (0x2f, 0x34, 0x3b), (0x16, 0x19, 0x1e)
for y in range(S):
    t = y / (S - 1)
    grad.putpixel((0, y), tuple(int(top[i] + (bot[i] - top[i]) * t) for i in range(3)))
grad = grad.resize((S, S)).convert("RGBA")
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=200, fill=255)
bg = Image.new("RGBA", (S, S), (0, 0, 0, 0))
bg.paste(grad, (0, 0), mask)

# --- USB stick desenhado na horizontal, depois rotacionado ---
L = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ld = ImageDraw.Draw(L)
bh = 230
by0, by1 = cy - bh // 2, cy + bh // 2
body_x0, body_x1 = 360, 824
con_x0, con_x1 = 232, 360

# conector metálico (gradiente prata) + recortes dos contatos
for x in range(con_x0, con_x1):
    t = (x - con_x0) / (con_x1 - con_x0)
    c = int(0xC4 + (0xEE - 0xC4) * t)
    ld.line([(x, by0 + 34), (x, by1 - 34)], fill=(c, c, c, 255))
ld.rounded_rectangle([con_x0, by0 + 34, con_x1, by1 - 34], radius=12,
                     outline=(0x80, 0x84, 0x88, 255), width=5)
ld.rectangle([con_x0 + 34, cy - 42, con_x1 - 16, cy - 8], fill=(0x52, 0x56, 0x5a, 255))
ld.rectangle([con_x0 + 34, cy + 8, con_x1 - 16, cy + 42], fill=(0x52, 0x56, 0x5a, 255))

# corpo escuro
ld.rounded_rectangle([body_x0, by0, body_x1, by1], radius=44, fill=(0x3c, 0x41, 0x47, 255))
# janela do slider
ld.rounded_rectangle([body_x0 + 64, by0 + 46, body_x1 - 70, by1 - 46], radius=26,
                     fill=(0x2a, 0x2e, 0x33, 255))
# botão do slider (claro)
ld.rounded_rectangle([body_x0 + 150, by0 + 72, body_x0 + 330, by1 - 72], radius=22,
                     fill=(0xc9, 0xcf, 0xd5, 255))
# brilho superior sutil
ld.line([(body_x0 + 36, by0 + 14), (body_x1 - 40, by0 + 14)], fill=(255, 255, 255, 46), width=6)

L = L.rotate(28, resample=Image.BICUBIC, center=(cx, cy))

# sombra projetada
sil = Image.new("RGBA", (S, S), (0, 0, 0, 0))
sil.paste((0, 0, 0, 150), (0, 0), L.split()[3])
sil = sil.filter(ImageFilter.GaussianBlur(20))
shadow_layer = Image.new("RGBA", (S, S), (0, 0, 0, 0))
shadow_layer.alpha_composite(sil, (12, 20))
shadow_layer.putalpha(shadow_layer.split()[3].point(lambda a: int(a)))
bg.alpha_composite(Image.composite(shadow_layer, Image.new("RGBA", (S, S)), mask))

bg.alpha_composite(L)
out = __file__.rsplit("/", 1)[0] + "/rufus-icon.png"
bg.save(out)
print("wrote", out)
