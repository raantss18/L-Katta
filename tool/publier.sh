#!/usr/bin/env bash
# Publie la version de Cargo.toml, signée : tests, AppImage, archive, empreintes, signature
# Ed25519 de SHA256SUMS, étiquette Git, version GitHub.
#
#   tool/publier.sh notes.md        (notes de version, en Markdown)
#
# La clé privée est dans release-keys/ed25519.pem (hors Git). Sans elle, rien n'est publié :
# les L Katta installés refuseraient une version non signée. Garde une copie de ce fichier
# hors de l'ordinateur ; si elle est perdue, plus aucune mise à jour automatique n'est possible.
set -euo pipefail
cd "$(dirname "$0")/.."

notes="${1:?usage : tool/publier.sh notes.md}"
cle="release-keys/ed25519.pem"
v="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
[ -f "$notes" ] || { echo "notes introuvables : $notes" >&2; exit 1; }
[ -f "$cle" ] || { echo "clé de signature absente : $cle" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "des changements ne sont pas commités" >&2; exit 1; }
if git rev-parse "v$v" >/dev/null 2>&1; then echo "l'étiquette v$v existe déjà" >&2; exit 1; fi
grep -q "^## $v " CHANGELOG.md || { echo "CHANGELOG.md n'a pas d'entrée $v" >&2; exit 1; }

cargo test --locked
RUSTFLAGS="-D warnings" cargo build --locked
appimage="$(bash scripts/appimage.sh | tail -n1)"

d="dist/lkatta-$v-x86_64-linux"
rm -rf "$d" && mkdir -p "$d/docs" "$d/fonts"
cp target/release/lkatta README.md NOTICE CHANGELOG.md LICENSE "$d/"
cp docs/*.md docs/*.png "$d/docs/"
cp assets/fonts/OFL-*.txt "$d/fonts/"
(
  cd dist
  tar czf "lkatta-$v-x86_64-linux.tar.gz" "lkatta-$v-x86_64-linux"
  sha256sum "$(basename "$appimage")" "lkatta-$v-x86_64-linux.tar.gz" > SHA256SUMS
)
# Signature : 64 octets Ed25519 sur le contenu exact de SHA256SUMS, en base64.
openssl pkeyutl -sign -inkey "$cle" -rawin -in dist/SHA256SUMS | base64 -w0 > dist/SHA256SUMS.sig
echo >> dist/SHA256SUMS.sig
# Contrôle par le binaire lui-même, avec la clé publique qu'il embarque.
target/release/lkatta verifier-signature dist/SHA256SUMS dist/SHA256SUMS.sig

git tag "v$v"
git push origin main "v$v"
gh release create "v$v" "$appimage" "dist/lkatta-$v-x86_64-linux.tar.gz" dist/SHA256SUMS dist/SHA256SUMS.sig --title "L Katta $v" --notes-file "$notes"
echo "Publié : v$v (signée)"
