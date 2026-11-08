// Seul script du site. Sans lui, la page reste complète : seuls le lémurien qui se promène et le bouton « Copier »
// manquent.
'use strict';

// La langue de la page (<html lang>) choisit les textes : français par défaut, anglais sur /en/.
var LANGUE = /^en\b/i.test(document.documentElement.lang || '') ? 'en' : 'fr';
// Dossier du site, déduit de l'adresse de ce script : la page anglaise (/en/) le charge depuis ../app.js.
var RACINE = (function () {
  var s = document.currentScript;
  try { return s && s.src ? new URL('.', s.src).href : ''; } catch (e) { return ''; }
})();

var TEXTES = {
  fr: {
    copie: 'Copié',
    bonjour: "Salut ! Moi, c'est L Katta. D'habitude, je vis sur un bureau Linux.",
    phrases: [
      "Je réponds dans une bulle, puis je retourne à mes affaires.",
      "Je reste sur les bords : ta zone de travail est à toi.",
      "Un clic droit sur moi, et je me fais oublier.",
      "Je ne prends jamais le clavier de moi-même.",
      "Tes clés vont dans le trousseau, jamais dans un fichier.",
      "Six lémuriens au choix. Lequel te ressemble ?"
    ],
    presentations: {}   // celles de lemuriens.json, écrites en français par l'application
  },
  en: {
    copie: 'Copied',
    bonjour: "Hi! I'm L Katta. I usually live on a Linux desktop.",
    phrases: [
      "I answer in a bubble, then go back to my own business.",
      "I stay near the edges: your workspace is yours.",
      "Right-click me, and I'll leave you be.",
      "I never take over your keyboard on my own.",
      "Your keys go in the keyring, never in a file.",
      "Six lemurs to choose from. Which one is most like you?"
    ],
    presentations: {
      catta: 'Here I am as a ring-tailed lemur: tail up, ready to go!',
      sifaka: 'Here I am as a sifaka: watch me dance sideways!',
      indri: 'Here I am as an indri: the biggest of all lemurs, and I sing!',
      ayeaye: 'Here I am as an aye-aye: big ears, and a finger that taps.',
      microcebe: "Here I am as a mouse lemur: tiny, but I don't miss a thing!",
      vari: 'Here I am as a ruffed lemur: careful, I can get really loud!'
    }
  }
}[LANGUE];

var DOSSIER = RACINE + 'img/lemuriens/';
function lire(cle) { try { return localStorage.getItem(cle); } catch (e) { return null; } }
function ecrire(cle, v) { try { localStorage.setItem(cle, v); } catch (e) { /* stockage indisponible : tant pis */ } }

// Bouton « Copier » de l'empreinte SHA-256.
(function () {
  if (!navigator.clipboard) return;
  document.querySelectorAll('[data-copier]').forEach(function (bouton) {
    var cible = document.getElementById(bouton.dataset.copier);
    if (!cible) return;
    var texte = bouton.textContent;
    bouton.hidden = false;
    bouton.addEventListener('click', function () {
      navigator.clipboard.writeText(cible.textContent.trim()).then(function () {
        bouton.textContent = TEXTES.copie;
        setTimeout(function () { bouton.textContent = texte; }, 2000);
      });
    });
  });
})();

// Le maki de l'accueil prend l'espèce choisie.
function majAccueil(id) {
  var m = document.querySelector('.maki-accueil');
  if (m) m.style.backgroundImage = 'url("' + DOSSIER + id + '-assis.png")';
}

// Le lémurien qui se promène en bas de la page. Ses images sont dessinées par l'application elle-même
// (`lkatta site-images`) : planches horizontales de vignettes, personnage tourné vers la droite, pieds en bas.
// Il est décoratif (aria-hidden) et discret : il n'arrive qu'une fois l'accueil dépassé, marche dans les marges,
// parle deux fois au plus de lui-même, dort quand on ne fait rien. Tout ce qu'il permet se fait aussi au clavier
// depuis le choix des lémuriens.
(function () {
  var scene = document.querySelector('.lemurien-scene');
  var el = document.getElementById('lemurien');
  var bulle = document.getElementById('bulle-lemurien');
  var annonce = document.getElementById('annonce');
  var radios = document.querySelectorAll('input[name="espece"]');
  var caseVisible = document.getElementById('promenade');
  var perche = document.querySelector('.perche');
  var colonne = document.querySelector('main');

  // Le choix d'espèce marche même sans planches animées : il change au moins le maki de l'accueil.
  var memorise = lire('lkatta-espece');
  radios.forEach(function (r) {
    if (memorise && r.value === memorise) { r.checked = true; majAccueil(memorise); }
  });

  if (!scene || !el || !bulle || !window.fetch || !window.requestAnimationFrame) {
    radios.forEach(function (r) { r.addEventListener('change', function () { ecrire('lkatta-espece', r.value); majAccueil(r.value); }); });
    return;
  }

  var MAX_SPONTANEES = 2;     // par visite : bonjour, puis une phrase, pas plus
  var DODO_APRES = 60;        // secondes sans souris, clavier ni défilement

  var mouvementReduit = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)');
  function reduit() { return !!(mouvementReduit && mouvementReduit.matches); }
  function hasard(a, b) { return a + Math.random() * (b - a); }

  var donnees = null, espece = null;
  var taille = 88;
  var x = 0, dir = -1, saut = 0;
  var etat = 'assis', debutEtat = 0, finEtat = 0, cible = 0;
  var prochainClin = 0, dernierSalut = -1e9, dernierNumero = 0;
  var derniereActivite = 0, prochainePhrase = 0, spontanees = 0, ordrePhrases = [];
  var finBulle = 0, enMarche = false, dernierT = 0, maintenant = 0;
  var loin = !perche;         // l'accueil (et son maki) est-il sorti de l'écran ?
  var cacheFond = '', cachePos = '', cacheTransfo = '';

  function anim(nom) { return espece.animations[nom]; }

  // Zones où il peut marcher : les marges de la colonne si elles sont assez larges, sinon toute la largeur.
  function zones() {
    var W = document.documentElement.clientWidth;
    var r = colonne.getBoundingClientRect();
    if (r.left >= taille + 12) return [[6, r.left - taille], [r.right, W - taille - 6]];
    return [[6, Math.max(6, W - taille - 6)]];
  }
  function zoneDe(px) {
    var zs = zones();
    for (var i = 0; i < zs.length; i++) if (px >= zs[i][0] - 1 && px <= zs[i][1] + 1) return zs[i];
    return zs[0];
  }
  function borner() {
    var zs = zones();
    x = Math.min(Math.max(x, zs[0][0]), zs[zs.length - 1][1]);
  }

  function changer(nouvel, duree) {
    etat = nouvel;
    debutEtat = maintenant;
    finEtat = maintenant + (duree || 0);
    el.classList.toggle('dort', etat === 'dodo');
  }

  function decider() {
    if (reduit()) { changer('assis', 1e9); return; }
    if (finBulle > maintenant) { changer('assis', finBulle - maintenant + 1000); return; }   // il se tait en marchant
    if ((maintenant - derniereActivite) / 1000 > DODO_APRES) { changer('dodo', 1e9); return; }
    var r = Math.random();
    if (r < 0.1 && maintenant - dernierNumero > 60000) {
      var n = anim('numero');
      dernierNumero = maintenant;
      changer('numero', 2 * n.images / n.ips * 1000);
      return;
    }
    if (r < 0.55) {
      var zs = zones(), z = zoneDe(x);
      if (zs.length > 1 && Math.random() < 0.1) z = (z === zs[0]) ? zs[1] : zs[0];
      cible = hasard(z[0], z[1]);
      if (Math.abs(cible - x) < 40) { changer('assis', hasard(5000, 10000)); return; }
      dir = cible > x ? 1 : -1;
      changer('marche', 1e9);
      return;
    }
    changer('assis', hasard(6000, 14000));
  }

  function saluer() {
    if (reduit()) return;
    var a = anim('salut');
    dernierSalut = maintenant;
    changer('salut', a.images / a.ips * 1000);
  }

  function dire(texte, annoncer) {
    bulle.textContent = texte;
    bulle.classList.add('visible');
    finBulle = maintenant + 4000 + texte.length * 40;
    if (etat === 'marche' || etat === 'dodo') changer('assis', finBulle - maintenant + 1000);
    if (annoncer && annonce) annonce.textContent = texte;
    placerBulle();
  }

  function phraseSuivante() {
    if (!ordrePhrases.length) ordrePhrases = TEXTES.phrases.slice().sort(function () { return Math.random() - 0.5; });
    return ordrePhrases.pop();
  }

  function placerBulle() {
    var W = document.documentElement.clientWidth;
    var largeur = bulle.offsetWidth;
    var centre = x + taille / 2;
    var gauche = Math.min(Math.max(centre - largeur / 2, 8), W - largeur - 8);
    bulle.style.left = gauche + 'px';
    bulle.style.setProperty('--queue', Math.min(Math.max(centre - gauche, 16), largeur - 16) + 'px');
  }

  function dessiner() {
    var a = anim(etat);
    var t = (maintenant - debutEtat) / 1000;
    var image = 0;
    if (etat === 'assis') {
      // Il cligne des yeux de temps en temps (deuxième image), pas en boucle.
      if (!reduit() && maintenant >= prochainClin) {
        if (maintenant < prochainClin + 160) image = 1;
        else prochainClin = maintenant + hasard(2500, 6000);
      }
    } else if (etat === 'salut' || etat === 'numero') {
      image = Math.min(Math.floor(t * a.ips) % a.images, a.images - 1);
    } else if (a.images > 1) {
      image = Math.floor(t * a.ips) % a.images;
    }
    var fond = a.fichier + '|' + a.images + '|' + taille;
    if (fond !== cacheFond) {
      cacheFond = fond;
      el.style.backgroundImage = 'url("' + DOSSIER + a.fichier + '")';
      el.style.backgroundSize = (a.images * taille) + 'px ' + taille + 'px';
    }
    var pos = (-image * taille) + 'px 0';
    if (pos !== cachePos) { cachePos = pos; el.style.backgroundPosition = pos; }
    var transfo = 'translate3d(' + x.toFixed(1) + 'px,' + (-saut).toFixed(1) + 'px,0) scaleX(' + dir + ')';
    if (transfo !== cacheTransfo) { cacheTransfo = transfo; el.style.transform = transfo; }
    if (bulle.classList.contains('visible')) placerBulle();
  }

  function tick(t) {
    if (!enMarche) return;
    var dt = Math.min((t - (dernierT || t)) / 1000, 0.1);
    dernierT = t;
    maintenant = t;
    saut = 0;

    if (etat === 'marche') {
      var pas = espece.vitesse * (taille / 120) * dt;
      if (Math.abs(cible - x) <= pas) { x = cible; changer('assis', hasard(5000, 12000)); }
      else x += dir * pas;
    } else if (etat === 'vol') {
      var p = Math.min((maintenant - debutEtat) / (finEtat - debutEtat), 1);
      saut = Math.sin(Math.PI * p) * taille * 0.4;
      if (p >= 1) changer('assis', hasard(2500, 5000));
    } else if (maintenant >= finEtat) {
      decider();
    }

    if (finBulle && maintenant > finBulle) { bulle.classList.remove('visible'); finBulle = 0; }
    if (!reduit() && spontanees < MAX_SPONTANEES && maintenant > prochainePhrase && etat !== 'dodo' && !finBulle) {
      spontanees++;
      prochainePhrase = maintenant + hasard(45000, 75000);
      dire(spontanees === 1 ? TEXTES.bonjour : phraseSuivante());
    }
    dessiner();
    requestAnimationFrame(tick);
  }

  // Il ne tourne que visible : accueil dépassé, promenade permise, onglet affiché.
  var petitEcran = window.matchMedia && window.matchMedia('(max-width: 700px)');   // il n'y est pas affiché (style.css)
  function demarrer() {
    if (enMarche || scene.hidden || !espece || !loin || document.hidden || (petitEcran && petitEcran.matches)) return;
    enMarche = true;
    dernierT = 0;
    requestAnimationFrame(tick);
  }
  function arreter() { enMarche = false; }

  function mesurer() {
    taille = el.offsetWidth || 88;
    cacheFond = '';
    borner();
  }

  // Changer d'espèce : précharger ses planches, puis il arrive d'un petit bond.
  function choisir(id, presenter) {
    var e = donnees.lemuriens.filter(function (l) { return l.id === id; })[0] || donnees.lemuriens[0];
    var noms = Object.keys(e.animations);
    var restant = noms.length;
    noms.forEach(function (n) {
      var img = new Image();
      img.onload = img.onerror = function () {
        if (--restant) return;
        espece = e;
        mesurer();
        el.classList.add('pret');
        maintenant = performance.now();
        prochainClin = maintenant + hasard(2000, 4000);
        if (reduit()) changer('assis', 1e9);
        else { dir = x > document.documentElement.clientWidth / 2 ? -1 : 1; changer('vol', 650); }
        if (presenter && loin && !scene.hidden && !(petitEcran && petitEcran.matches)) dire(TEXTES.presentations[e.id] || e.presentation || '', true);
        dessiner();
        demarrer();
      };
      img.src = DOSSIER + e.animations[n].fichier;
    });
  }

  function montrer(oui) {
    scene.hidden = !oui;
    ecrire('lkatta-cache', oui ? '0' : '1');
    if (oui) { if (espece) mesurer(); demarrer(); } else { arreter(); bulle.classList.remove('visible'); finBulle = 0; }
  }

  function activite() {
    derniereActivite = performance.now();
    if (etat === 'dodo') { maintenant = derniereActivite; changer('assis', hasard(2000, 4000)); }
  }

  fetch(DOSSIER + 'lemuriens.json')
    .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
    .then(function (d) {
      if (!d || !d.lemuriens || !d.lemuriens.length) throw new Error('vide');
      donnees = d;

      radios.forEach(function (r) {
        r.addEventListener('change', function () {
          if (!r.checked) return;
          ecrire('lkatta-espece', r.value);
          majAccueil(r.value);
          choisir(r.value, true);
        });
      });
      if (caseVisible) {
        if (lire('lkatta-cache') === '1') { scene.hidden = true; caseVisible.checked = false; }
        caseVisible.closest('.promenade').hidden = false;
        caseVisible.addEventListener('change', function () { montrer(caseVisible.checked); });
      }

      maintenant = derniereActivite = performance.now();
      prochainePhrase = maintenant + 6000;
      taille = el.offsetWidth || 88;
      var zs = zones();
      x = zs[zs.length - 1][1];
      choisir(memorise || 'catta', false);

      // Le maki de l'accueil et celui du bas ne se montrent jamais ensemble.
      if (perche && window.IntersectionObserver) {
        new IntersectionObserver(function (entrees) {
          loin = !entrees[0].isIntersecting;
          scene.classList.toggle('loin', loin);
          if (loin) { maintenant = performance.now(); prochainePhrase = Math.max(prochainePhrase, maintenant + 4000); demarrer(); }
          else { arreter(); bulle.classList.remove('visible'); finBulle = 0; }
        }).observe(perche);
      } else {
        loin = true;
        scene.classList.add('loin');
      }

      el.addEventListener('mouseenter', function () {
        if (!espece) return;
        maintenant = performance.now();
        activite();
        if (etat !== 'salut' && etat !== 'vol' && maintenant - dernierSalut > 4000) saluer();
      });
      el.addEventListener('click', function () {
        if (!espece) return;
        maintenant = performance.now();
        activite();
        saluer();
        dire(phraseSuivante());
      });
      ['mousemove', 'keydown', 'scroll', 'touchstart', 'wheel'].forEach(function (ev) {
        window.addEventListener(ev, activite, { passive: true });
      });
      window.addEventListener('resize', function () { if (espece) mesurer(); });
      document.addEventListener('visibilitychange', function () { if (document.hidden) arreter(); else demarrer(); });
      if (petitEcran && petitEcran.addEventListener) {
        petitEcran.addEventListener('change', function () { if (petitEcran.matches) arreter(); else { mesurer(); demarrer(); } });
      }
      if (mouvementReduit && mouvementReduit.addEventListener) {
        mouvementReduit.addEventListener('change', function () { if (espece) { maintenant = performance.now(); decider(); } });
      }
    })
    .catch(function () {
      scene.hidden = true;
      radios.forEach(function (r) { r.addEventListener('change', function () { ecrire('lkatta-espece', r.value); majAccueil(r.value); }); });
    });
})();
