"""Génère installer/conditions.rtf (page « Conditions » de l'installeur) depuis installer/conditions.txt.

Usage : python scripts/conditions-rtf.py
Texte foncé sur fond blanc (le fond est posé par MUI_LICENSEPAGE_BGCOLOR dans hooks.nsh). Les titres « ## … »
sont en gras et en violet. Relancer après chaque modification de conditions.txt (le texte de l'application le lit
directement).
"""
import os

ici = os.path.join(os.path.dirname(__file__), '..', 'src-tauri', 'installer')
lignes = open(os.path.join(ici, 'conditions.txt'), encoding='utf-8').read().splitlines()


def esc(t):
    out = []
    for c in t:
        o = ord(c)
        if c in '\\{}':
            out.append('\\' + c)
        elif o < 128:
            out.append(c)
        else:
            out.append(f'\\u{o if o < 32768 else o - 65536}?')
    return ''.join(out)


corps = []
for i, l in enumerate(lignes):
    if i == 0:
        corps.append(r'{\b\cf2\fs22 ' + esc(l) + r'}\par')
    elif l.startswith('## '):
        corps.append(r'\par{\b\cf2 ' + esc(l[3:]) + r'}\par')
    elif l.startswith('- '):
        corps.append(r'\bullet  ' + esc(l[2:]) + r'\par')
    elif l.strip():
        corps.append(esc(l) + r'\par')
rtf = (r'{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss Segoe UI;}}'
       r'{\colortbl;\red26\green27\blue46;\red91\green33\blue182;}'
       r'\viewkind4\uc1\pard\sa60\f0\fs17\cf1 ' + '\n'.join(corps) + '}\n')
open(os.path.join(ici, 'conditions.rtf'), 'w', encoding='ascii', newline='\r\n').write(rtf)
print('installer/conditions.rtf écrit')
