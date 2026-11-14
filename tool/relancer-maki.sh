#!/usr/bin/env bash
# Relance le maki avec ~/.local/bin/lkatta remplacé par $1, SEULEMENT si son service ne contient
# rien d'autre que le maki et ses moniteurs (un terminal lancé depuis la bulle y vivait avant 0.2.3).
set -u
unite=app-lkatta@autostart.service
autres=$(systemd-cgls --user-unit "$unite" 2>/dev/null | tail -n +2 | grep -vE "binfmt-interpreter|/usr/bin/lkatta|\.local/bin/lkatta|gdbus monitor|wl-paste|dbus-monitor")
if [ -n "$autres" ]; then
  echo "ARRÊT : d'autres processus vivent dans $unite, rien n'est relancé :"
  echo "$autres"
  exit 3
fi
cp "$1" ~/.local/bin/lkatta.nouveau && chmod +x ~/.local/bin/lkatta.nouveau && mv -f ~/.local/bin/lkatta.nouveau ~/.local/bin/lkatta
systemctl --user restart "$unite"
sleep 6
systemctl --user is-active "$unite"
