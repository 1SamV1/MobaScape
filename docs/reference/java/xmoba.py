#!/usr/bin/env python3
"""Minimal X11 driver for 2006Scape dev clients.

Commands:
  list                       list windows matching 2006Scape
  geom <id|title>            print window geometry
  focus <id|title>           raise + focus window
  type <text>                type text into focused window
  key <keysym> [mods...]     press a key (e.g. Return, F5, a, space)
  click <x> <y> [button]     click at absolute screen coords
  move <x> <y>               move pointer
  shot <id|title> <path>     capture window to PNG (uses ImageMagick import)
  screen <path>              capture whole screen
"""
import subprocess
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

D = display.Display()


def windows():
    root = D.screen().root
    out = []
    for w in root.query_tree().children:
        try:
            name = w.get_wm_name()
            cls = w.get_wm_class()
        except Exception:
            continue
        if name:
            out.append((w.id, name, cls))
    return out


def find(selector):
    sel = selector.lower()
    for wid, name, _ in windows():
        if sel == str(wid).lower() or sel == hex(wid).lower() or sel in (name or '').lower():
            return wid
    return None


def geom(wid):
    w = D.create_resource_object('window', wid)
    g = w.get_geometry()
    t = w.translate_coords(D.screen().root, 0, 0)
    return g.width, g.height, -t.x, -t.y


def focus(wid):
    w = D.create_resource_object('window', wid)
    try:
        w.configure(stack_mode=X.Above)
    except Exception:
        pass
    try:
        w.set_input_focus(X.RevertToParent, X.CurrentTime)
    except Exception:
        D.set_input_focus(w, X.RevertToParent, X.CurrentTime)
    D.sync()
    time.sleep(0.1)


def keycode(name):
    sym = XK.string_to_keysym(name)
    if sym == 0 and len(name) == 1:
        sym = ord(name)
    return D.keysym_to_keycode(sym)


def tap(name, mods=()):
    mod_codes = [keycode(m) for m in mods]
    code = keycode(name)
    for m in mod_codes:
        xtest.fake_input(D, X.KeyPress, m)
    xtest.fake_input(D, X.KeyPress, code)
    xtest.fake_input(D, X.KeyRelease, code)
    for m in reversed(mod_codes):
        xtest.fake_input(D, X.KeyRelease, m)
    D.sync()
    time.sleep(0.03)


SHIFT = {'~': 'asciitilde', '!': 'exclam', '@': 'at', '#': 'numbersign',
         '$': 'dollar', '%': 'percent', '^': 'asciicircum', '&': 'ampersand',
         '*': 'asterisk', '(': 'parenleft', ')': 'parenright', '_': 'underscore',
         '+': 'plus', '{': 'braceleft', '}': 'braceright', '|': 'bar',
         ':': 'colon', '"': 'quotedbl', '<': 'less', '>': 'greater',
         '?': 'question'}

NAMES = {' ': 'space', '\n': 'Return', '\t': 'Tab', '-': 'minus', '=': 'equal',
         '[': 'bracketleft', ']': 'bracketright', '\\': 'backslash',
         ';': 'semicolon', "'": 'apostrophe', ',': 'comma', '.': 'period',
         '/': 'slash', '`': 'grave'}


def hold(name, seconds):
    code = keycode(name)
    xtest.fake_input(D, X.KeyPress, code)
    D.sync()
    time.sleep(float(seconds))
    xtest.fake_input(D, X.KeyRelease, code)
    D.sync()


def type_text(text):
    for ch in text:
        if ch in SHIFT:
            tap(SHIFT[ch], ('Shift_L',))
        elif ch.isupper():
            tap(ch.lower(), ('Shift_L',))
        else:
            tap(NAMES.get(ch, ch))


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    cmd = sys.argv[1]
    if cmd == 'list':
        for wid, name, cls in windows():
            if 'scape' in name.lower() or 'scape' in str(cls).lower():
                w, h, x, y = geom(wid)
                print(f'{wid} {x},{y} {w}x{h} {name}')
    elif cmd in ('geom', 'focus', 'shot'):
        wid = find(sys.argv[2])
        if wid is None:
            print('window not found')
            return 2
        if cmd == 'geom':
            print(geom(wid))
        elif cmd == 'focus':
            focus(wid)
            print('focused', hex(wid))
        else:
            focus(wid)
            time.sleep(0.2)
            subprocess.run(['import', '-window', hex(wid), sys.argv[3]], check=True)
            print('saved', sys.argv[3])
    elif cmd == 'type':
        type_text(sys.argv[2])
    elif cmd == 'key':
        tap(sys.argv[2], tuple(sys.argv[3:]))
    elif cmd == 'hold':
        hold(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else '1')
    elif cmd == 'click':
        x, y = int(sys.argv[2]), int(sys.argv[3])
        button = int(sys.argv[4]) if len(sys.argv) > 4 else 1
        xtest.fake_input(D, X.MotionNotify, x=x, y=y)
        D.sync()
        time.sleep(0.05)
        xtest.fake_input(D, X.ButtonPress, button)
        xtest.fake_input(D, X.ButtonRelease, button)
        D.sync()
    elif cmd == 'move':
        xtest.fake_input(D, X.MotionNotify, x=int(sys.argv[2]), y=int(sys.argv[3]))
        D.sync()
    elif cmd == 'screen':
        subprocess.run(['import', '-window', 'root', sys.argv[2]], check=True)
        print('saved', sys.argv[2])
    else:
        print(__doc__)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
