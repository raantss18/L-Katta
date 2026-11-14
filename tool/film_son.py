#!/usr/bin/env python3
"""Bande-son du film de présentation : une musique originale, synthétisée ici (cordes pincées
façon valiha, basse, nappe, shaker), et les sons de l'interface calés sur les instants notés par
`lkatta apercu-film` (fichier `<film>.sons.json`).

    lkatta apercu-film /tmp/muet.mp4
    python3 tool/film_son.py /tmp/muet.mp4 site/video/lkatta.mp4

Rien n'est repris d'ailleurs : la mélodie est écrite ci-dessous, note par note.
Dépendances : numpy, ffmpeg.
"""
import json
import subprocess
import sys
import wave

import numpy as np

SR = 44100
BPM = 96
TEMPS = 60 / BPM
# Ré majeur pentatonique (ré mi fa# la si), comme l'accord ouvert d'une valiha.
GAMME = [62, 64, 66, 69, 71]


def freq(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def degre(n):
    """n-ième note de la gamme pentatonique (0 = ré4), octaves comprises."""
    return GAMME[n % 5] + 12 * (n // 5)


def pincee(f, duree=1.6, brillance=1.0):
    """Corde pincée : partiels légèrement inharmoniques, attaque nette, extinction rapide."""
    t = np.arange(int(SR * duree)) / SR
    s = np.zeros_like(t)
    for k, (h, a, d) in enumerate([(1, 1.0, 3.2), (2.0, 0.42, 4.5), (3.01, 0.22, 6.5), (4.03, 0.12, 9.0), (5.9, 0.05, 14.0)]):
        s += a * (brillance if k else 1.0) * np.sin(2 * np.pi * f * h * t) * np.exp(-d * t)
    attaque = np.minimum(1.0, t / 0.004)
    return s * attaque


def nappe(f, duree):
    t = np.arange(int(SR * duree)) / SR
    s = sum(np.sin(2 * np.pi * f * m * t + p) / (i + 1) for i, (m, p) in enumerate([(1, 0), (1.003, 1.3), (2, 0.4), (0.5, 2.0)]))
    env = np.minimum(1.0, t / 1.2) * np.minimum(1.0, (duree - t) / 1.5)
    return s * env * 0.25


def shaker(duree=0.09, seed=0):
    rng = np.random.default_rng(seed)
    n = int(SR * duree)
    b = rng.standard_normal(n)
    b = np.diff(b, prepend=0)  # passe-haut simple
    return b * np.exp(-np.arange(n) / (SR * 0.02))


class Piste:
    def __init__(self, duree):
        self.g = np.zeros(int(SR * (duree + 4)))
        self.d = np.zeros_like(self.g)

    def poser(self, son, t, vol=1.0, pan=0.0):
        i = int(t * SR)
        if i < 0 or i >= len(self.g):
            return
        son = son[: len(self.g) - i]
        self.g[i : i + len(son)] += son * vol * (1 - max(0, pan))
        self.d[i : i + len(son)] += son * vol * (1 + min(0, pan))


def musique(duree, chapitres):
    p = Piste(duree)
    mesure = TEMPS * 4
    # Grille d'accords (fondamentales en degrés de ré majeur) : ré, si mineur, sol, la.
    accords = [(50, [0, 2, 3]), (47, [1, 3, 4]), (43, [0, 1, 3]), (45, [1, 2, 4])]
    # Motif d'arpège par mesure (croches) : indices dans l'accord, octave.
    motifs = [
        [(0, 0), (1, 0), (2, 0), (1, 0), (0, 1), (1, 0), (2, 0), (1, 0)],
        [(0, 0), (2, 0), (1, 0), (2, 0), (0, 1), (2, 0), (1, 1), (2, 0)],
    ]
    # Petite mélodie (degrés pentatoniques, durée en temps) reprise toutes les 8 mesures.
    melodie = [(7, 1.5), (6, 0.5), (5, 1), (7, 1), (9, 2), (8, 1), (7, 1), (5, 1.5), (6, 0.5), (7, 1), (4, 1), (5, 3), (None, 1),
               (9, 1.5), (8, 0.5), (7, 1), (9, 1), (10, 2), (9, 1), (7, 1), (8, 1.5), (7, 0.5), (5, 1), (6, 1), (5, 3), (None, 1)]
    debut = chapitres.get("jeu", 9.0)
    fin = duree - 9.0
    n_mes = int((fin - debut) / mesure)
    # Intro : nappe et quelques notes qui descendent.
    for i, (b, _) in enumerate(accords[:2] * 2):
        p.poser(nappe(freq(b + 12), min(mesure * 1.02, 3.2)), i * debut / 4, 0.5)
    for i, n in enumerate([9, 7, 5, 4, 5, 7, 9, 10]):
        p.poser(pincee(freq(degre(n) + 12), 2.2, 0.8), 0.6 + i * debut / 8.6, 0.16, pan=(-0.4 if i % 2 else 0.4))
    code = chapitres.get("code", 1e9)
    for m in range(n_mes):
        t0 = debut + m * mesure
        basse, notes = accords[m % 4]
        plein = m >= 4
        riche = t0 >= code
        p.poser(nappe(freq(basse + 12), mesure * 1.05), t0, 0.34 if plein else 0.22)
        # Basse : fondamentale, puis rappel.
        p.poser(pincee(freq(basse), 1.8, 0.4), t0, 0.5)
        if plein:
            p.poser(pincee(freq(basse + 7), 1.0, 0.4), t0 + TEMPS * 2.5, 0.26)
        # Arpège.
        motif = motifs[(m // 4) % 2]
        for k, (idx, octv) in enumerate(motif):
            n = degre(notes[idx]) + 12 * octv
            doux = 0.2 if k % 2 else 0.27
            p.poser(pincee(freq(n), 1.3), t0 + k * TEMPS / 2, doux * (1.0 if plein else 0.7), pan=-0.35 if k % 2 else 0.3)
            if riche and k % 4 == 2:
                p.poser(pincee(freq(n + 12), 0.9, 1.2), t0 + k * TEMPS / 2 + TEMPS / 4, 0.09, pan=0.5)
        # Shaker léger sur les contretemps.
        if plein:
            for k in range(8):
                p.poser(shaker(seed=m * 8 + k), t0 + k * TEMPS / 2 + (TEMPS / 4 if k % 2 == 0 else 0), 0.035 if k % 2 else 0.02, pan=0.2)
    # Mélodie, à partir de la 5e mesure, une phrase sur deux pour laisser respirer.
    t = debut + 4 * mesure
    i = 0
    while t < fin - mesure:
        n, d = melodie[i % len(melodie)]
        phrase = (i // 13) % 2 == 0 or t >= code
        if n is not None and phrase:
            p.poser(pincee(freq(degre(n) + 12), 2.4, 0.9), t, 0.2, pan=0.1)
        t += d * TEMPS
        i += 1
    # Fin : accord de ré qui s'ouvre et s'éteint.
    for k, n in enumerate([0, 2, 3, 5, 7, 10]):
        p.poser(pincee(freq(degre(n)), 5.0, 0.8), fin + k * 0.22, 0.24, pan=(k % 3 - 1) * 0.4)
    p.poser(nappe(freq(62), 8.0), fin, 0.5)
    p.poser(pincee(freq(38), 5.0, 0.3), fin, 0.55)
    return p


def effets(p, sons):
    for s in sons:
        t, g = s["t"], s["son"]
        if g == "touche":
            x = shaker(0.03, seed=int(t * 1000)) * 0.5
            p.poser(x, t, 0.05, pan=0.2)
        elif g == "clic":
            n = int(SR * 0.05)
            x = np.sin(2 * np.pi * 1900 * np.arange(n) / SR) * np.exp(-np.arange(n) / (SR * 0.006))
            p.poser(x, t, 0.22)
            p.poser(shaker(0.02, seed=3), t, 0.12)
        elif g == "envoi":
            for k, m in enumerate([74, 81]):
                p.poser(pincee(freq(m), 0.5, 1.4), t + k * 0.07, 0.13, pan=0.3)
        elif g == "reponse":
            for k, m in enumerate([86, 81]):
                p.poser(pincee(freq(m), 0.9, 1.0), t + k * 0.09, 0.12, pan=-0.3)
        elif g == "ouvre":
            n = int(SR * 0.18)
            tt = np.arange(n) / SR
            x = np.sin(2 * np.pi * (300 + 900 * tt / 0.18) * tt) * np.exp(-tt / 0.06)
            p.poser(x, t, 0.2)
        elif g == "succes":
            for k, m in enumerate([74, 78, 81, 86, 90]):
                p.poser(pincee(freq(m), 1.6, 1.2), t + k * 0.09, 0.2, pan=(k - 2) * 0.2)


def reverbe(x, retards=(0.031, 0.047, 0.071, 0.113), gain=0.22):
    y = x.copy()
    for r in retards:
        n = int(SR * r)
        e = np.zeros_like(x)
        e[n:] = x[:-n]
        e2 = np.zeros_like(x)
        e2[2 * n :] = x[: -2 * n]
        y += gain * e + gain * 0.55 * e2
    return y


def main():
    muet, sortie = sys.argv[1], sys.argv[2]
    d = json.load(open(muet + ".sons.json"))
    duree = d["duree"]
    # Le jeu commence quand la bulle s'ouvre (premier clic) ; le côté Code enrichit l'arpège.
    clics = [s["t"] for s in d["sons"] if s["son"] == "clic"]
    ouvre = [s["t"] for s in d["sons"] if s["son"] == "ouvre"]
    chap = {"jeu": 9.2, "code": ouvre[1] if len(ouvre) > 1 else 1e9}
    _ = clics
    p = musique(duree, chap)
    # La musique passe sous la bulle : un peu plus douce pendant les scènes, pleine au début et à la fin.
    n = len(p.g)
    t = np.arange(n) / SR
    niveau = np.where((t > 12) & (t < duree - 9), 0.62, 1.0)
    c = np.cumsum(np.concatenate([[0.0], niveau]))
    lisse = (c[np.minimum(n, np.arange(n) + SR // 2)] - c[np.maximum(0, np.arange(n) - SR // 2)]) / SR
    lisse = np.clip(lisse, 0.62, 1.0)
    g, dr = p.g * lisse, p.d * lisse
    fx = Piste(duree)
    effets(fx, d["sons"])
    g, dr = reverbe(g) + fx.g + 0.3 * (reverbe(fx.g) - fx.g), reverbe(dr) + fx.d + 0.3 * (reverbe(fx.d) - fx.d)
    st = np.stack([g, dr], axis=1)[: int(SR * duree)]
    # Fondus d'entrée et de sortie, puis mise à niveau.
    k = np.arange(len(st)) / SR
    st *= (np.minimum(1, k / 0.8) * np.minimum(1, (duree - k) / 3.0))[:, None]
    st *= 0.82 / max(1e-9, np.abs(st).max())
    wav = sortie + ".wav"
    with wave.open(wav, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes((st * 32767).astype("<i2").tobytes())
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", muet, "-i", wav, "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest", "-movflags", "+faststart", sortie], check=True)
    subprocess.run(["rm", "-f", wav])
    print(sortie)


if __name__ == "__main__":
    main()
