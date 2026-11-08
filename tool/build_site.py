#!/usr/bin/env python3
"""Construit le site vitrine (dossier site/) dans un dossier de sortie, prêt pour GitHub Pages.

Le site est du HTML statique. Ce script y écrit ce qui change à chaque version, à partir de la liste des versions publiées
sur GitHub (JSON de `gh api repos/<dépôt>/releases`) : numéro, date, taille, empreinte, liens de téléchargement de la
dernière version qui a une AppImage, et les nouveautés des 5 dernières versions. Il écrit aussi version.json.
Les images du lémurien (img/lemuriens/) sont dessinées ensuite par l'application : `lkatta site-images _site/img/lemuriens`.
Aucune bibliothèque : Python 3 seulement.

Usage : python3 tool/build_site.py --releases releases.json --out _site
"""
import argparse
import hashlib
import html
import json
import re
import shutil
from datetime import datetime, timezone
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
SITE = RACINE / 'site'
DEPOT = 'https://github.com/raantss18/L-Katta'
MOIS = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre']
NB_NOUVEAUTES = 5


def date_fr(iso: str) -> str:
    d = datetime.fromisoformat(iso.replace('Z', '+00:00'))
    return f"{d.day}{'er' if d.day == 1 else ''} {MOIS[d.month - 1]} {d.year}"


def taille_fr(octets: int) -> str:
    return f"{octets / 1e6:.1f}".replace('.', ',') + ' Mo'


def e(texte: str) -> str:
    return html.escape(texte, quote=True)


def publiees(releases):
    """Versions publiées (ni brouillon ni préversion), la plus récente d'abord."""
    rs = [r for r in releases if not r.get('draft') and not r.get('prerelease') and r.get('published_at')]
    return sorted(rs, key=lambda r: r['published_at'], reverse=True)


def numero(r) -> str:
    return r['tag_name'].removeprefix('v')


def derniere_appimage(rs):
    """La dernière version publiée qui contient un fichier *-x86_64.AppImage, ou None."""
    for r in rs:
        appimage = next((a for a in r.get('assets', []) if a['name'].endswith('-x86_64.AppImage')), None)
        if not appimage:
            continue
        archive = next((a for a in r.get('assets', []) if a['name'].endswith('.tar.gz')), None)
        return {
            'version': numero(r),
            'date': r['published_at'],
            'url': r['html_url'],
            'appimage': appimage['browser_download_url'],
            'appimage_nom': appimage['name'],
            'taille': appimage['size'],
            'sha256': (appimage.get('digest') or '').removeprefix('sha256:'),
            'archive': archive['browser_download_url'] if archive else None,
        }
    return None


def en_ligne(texte: str) -> str:
    """Markdown en ligne → HTML échappé : `code` et **gras**. Le texte est échappé avant toute balise."""
    morceaux = re.split(r'(`[^`]+`)', texte)
    out = []
    for m in morceaux:
        if len(m) > 1 and m.startswith('`') and m.endswith('`'):
            out.append(f'<code>{e(m[1:-1])}</code>')
        else:
            out.append(re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', e(m)))
    return ''.join(out)


def markdown(corps: str) -> str:
    """Markdown simple des notes de version : titres, paragraphes, listes à puces (avec suite de ligne), gras, code
    en ligne et blocs de code entre ```."""
    blocs, liste, para, code = [], [], [], None

    def fermer_para():
        if para:
            blocs.append(f"<p>{en_ligne(' '.join(para))}</p>")
            para.clear()

    def fermer_liste():
        if liste:
            blocs.append('<ul>' + ''.join(f'<li>{en_ligne(li)}</li>' for li in liste) + '</ul>')
            liste.clear()

    for brute in corps.replace('\r\n', '\n').split('\n'):
        ligne = brute.strip()
        if code is not None:
            if ligne.startswith('```'):
                blocs.append(f"<pre><code>{e(chr(10).join(code))}</code></pre>")
                code = None
            else:
                code.append(brute.rstrip())
            continue
        puce = re.match(r'^[-*+]\s+(.*)$', ligne)
        titre = re.match(r'^#{1,6}\s+(.*)$', ligne)
        if ligne.startswith('```'):
            fermer_para()
            fermer_liste()
            code = []
        elif titre:
            fermer_para()
            fermer_liste()
            blocs.append(f"<h4>{en_ligne(titre[1])}</h4>")
        elif not ligne:
            fermer_para()
            fermer_liste()
        elif puce:
            fermer_para()
            liste.append(puce[1])
        elif liste and brute[:1] in (' ', '\t'):
            liste[-1] += ' ' + ligne      # suite d'une puce sur la ligne suivante
        else:
            fermer_liste()
            para.append(ligne)
    if code is not None:  # bloc de code jamais fermé : on le montre quand même
        blocs.append(f"<pre><code>{e(chr(10).join(code))}</code></pre>")
    fermer_para()
    fermer_liste()
    return ''.join(blocs)


def html_nouveautes(rs) -> str:
    if not rs:
        return ('<p>Aucune version publiée pour le moment. '
                f'<a href="{DEPOT}">Suis le projet sur GitHub</a> pour la voir arriver.</p>')
    blocs = []
    for r in rs[:NB_NOUVEAUTES]:
        corps = markdown((r.get('body') or '').strip()) or '<p>Nouvelle version.</p>'
        blocs.append(
            f"<article class=\"nouvelle\"><h3>Version {e(numero(r))}</h3>"
            f"<time datetime=\"{e(r['published_at'][:10])}\">{date_fr(r['published_at'])}</time>{corps}"
            f"<p><a href=\"{e(r['html_url'])}\">Cette version sur GitHub</a></p></article>")
    blocs.append(f'<p><a href="{DEPOT}/releases">Toutes les versions</a></p>')
    return '\n  '.join(blocs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--releases', required=True, help='JSON de `gh api repos/<dépôt>/releases`')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()

    try:
        releases = json.loads(Path(a.releases).read_text(encoding='utf-8'))
    except (OSError, ValueError) as err:
        print(f'Liste des versions illisible ({err}) : site construit sans version.')
        releases = []
    if not isinstance(releases, list):  # réponse d'erreur de l'API, par exemple
        releases = []
    rs = publiees(releases)
    dernier = derniere_appimage(rs)

    out = Path(a.out)
    if out.exists():
        shutil.rmtree(out)
    # Les images du lémurien sont redessinées par l'application dans le dossier de sortie : jamais de vieilles planches.
    shutil.copytree(SITE, out, ignore=shutil.ignore_patterns('lemuriens'))

    if dernier:
        valeurs = {
            'VERSION': e(dernier['version']),
            'DATE': date_fr(dernier['date']),
            'TAILLE': taille_fr(dernier['taille']),
            'APPIMAGE_URL': e(dernier['appimage']),
            'APPIMAGE_NOM': e(dernier['appimage_nom']),
            'SHA256': e(dernier['sha256'] or 'non fournie'),
            'ARCHIVE_URL': e(dernier['archive'] or dernier['url']),
            'RELEASE_URL': e(dernier['url']),
        }
    else:
        valeurs = {
            'VERSION': 'à venir', 'DATE': 'bientôt', 'TAILLE': 'taille inconnue',
            'APPIMAGE_URL': f'{DEPOT}/releases', 'APPIMAGE_NOM': 'LKatta-*-x86_64.AppImage',
            'SHA256': 'pas encore de fichier publié', 'ARCHIVE_URL': f'{DEPOT}/releases', 'RELEASE_URL': f'{DEPOT}/releases',
        }
    valeurs['NOUVEAUTES'] = html_nouveautes(rs)

    page = (SITE / 'index.html').read_text(encoding='utf-8')
    for cle, val in valeurs.items():
        page = page.replace('{{' + cle + '}}', val)
    # Chaque version du style et du script a son adresse : le navigateur ne garde jamais l'ancien en cache.
    for f in ['style.css', 'app.js']:
        h = hashlib.sha256((SITE / f).read_bytes()).hexdigest()[:10]
        page = page.replace(f'"{f}"', f'"{f}?v={h}"')
    if '{{' in page:
        raise SystemExit('Un champ du modèle n\'a pas de valeur : ' + page[page.index('{{'):][:40])
    (out / 'index.html').write_text(page, encoding='utf-8')

    version = {
        'version': dernier['version'] if dernier else None,
        'date': dernier['date'] if dernier else None,
        'appimage': dernier['appimage'] if dernier else None,
        'taille': dernier['taille'] if dernier else None,
        'sha256': (dernier['sha256'] or None) if dernier else None,
        'archive': dernier['archive'] if dernier else None,
        'notes': dernier['url'] if dernier else f'{DEPOT}/releases',
        'construit': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
    }
    (out / 'version.json').write_text(json.dumps(version, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (out / '.nojekyll').write_text('', encoding='utf-8')
    if dernier:
        print(f"Site construit dans {out} : version {dernier['version']}, {len(rs)} version(s) publiée(s).")
    else:
        print(f'Site construit dans {out} sans version téléchargeable (aucune AppImage publiée).')


if __name__ == '__main__':
    main()
