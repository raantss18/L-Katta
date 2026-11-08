// Seul script du site. Sans lui, la page reste complète : seuls le lémurien animé et les boutons « Copier » manquent.
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
    copier: 'Copier',
    copie: 'Copié',
    phrases: [
      "Je réponds dans une bulle, puis je retourne à mes affaires.",
      "Mon IA est gratuite, et il ne faut aucun compte.",
      "Je peux résumer tes mails sans les marquer comme lus.",
      "Un rendez-vous dans un quart d'heure ? Je te préviens.",
      "Je reste sur les bords : ta zone de travail est à toi.",
      "Pour sudo, c'est toi qui tapes le mot de passe. Toujours.",
      "Tes clés vont dans le trousseau, jamais dans un fichier.",
      "Je tiens dans un seul fichier : une AppImage.",
      "Un clic droit sur moi, et je me fais oublier.",
      "Attrape-moi et lance-moi : je retombe sur mes pattes.",
      "Je fais pousser un arbre, et j'y grimpe.",
      "Je ne prends jamais le clavier de moi-même.",
      "Six lémuriens au choix. Lequel te ressemble ?"
    ],
    bonjour: "Salut ! Moi, c'est L Katta. D'habitude, je vis sur un bureau Linux.",
    legende: 'Choisis ton lémurien',
    direBonjour: 'Lui dire bonjour',
    promener: ' Le laisser se promener en bas de la page',
    especes: {}   // les noms et descriptions viennent de lemuriens.json, écrit en français par l'application
  },
  en: {
    copier: 'Copy',
    copie: 'Copied',
    phrases: [
      "I answer in a bubble, then go back to my own business.",
      "My AI is free, and you don't need an account.",
      "I can sum up your email without marking it as read.",
      "A meeting in fifteen minutes? I'll give you a heads-up.",
      "I stay near the edges: your workspace is yours.",
      "For sudo, you type the password yourself. Always.",
      "Your keys go in the keyring, never in a file.",
      "I fit in a single file: an AppImage.",
      "Right-click me, and I'll leave you be.",
      "Grab me and throw me: I land on my feet.",
      "I grow a tree, and then I climb it.",
      "I never take over your keyboard on my own.",
      "Six lemurs to choose from. Which one is most like you?"
    ],
    bonjour: "Hi! I'm L Katta. I usually live on a Linux desktop.",
    legende: 'Choose your lemur',
    direBonjour: 'Say hello',
    promener: ' Let it wander along the bottom of the page',
    especes: {
      catta: {
        nom: 'Ring-tailed lemur (maky)',
        description: 'striped tail held high, walks on all fours, loves stretching in the sun',
        presentation: "Here I am as a ring-tailed lemur: tail up, ready to go!"
      },
      sifaka: {
        nom: "Verreaux's sifaka",
        description: 'all white with a brown cap, moves upright in sideways hops: it dances',
        presentation: 'Here I am as a sifaka: watch me dance sideways!'
      },
      indri: {
        nom: 'Indri (babakoto)',
        description: 'the biggest one, black and white, almost no tail, leaps upright and sings',
        presentation: 'Here I am as a babakoto: the biggest of all lemurs, and I sing!'
      },
      ayeaye: {
        nom: 'Aye-aye',
        description: 'big ears, big eyes, a long tapping finger; slow, mostly active at night',
        presentation: 'Here I am as an aye-aye: big ears, and a finger that taps.'
      },
      microcebe: {
        nom: 'Mouse lemur',
        description: 'tiny, reddish-brown, huge eyes; scurries in short bursts, mostly active at night',
        presentation: "Here I am as a mouse lemur: tiny, but I don't miss a thing!"
      },
      vari: {
        nom: 'Black-and-white ruffed lemur (varikandana)',
        description: 'black-and-white ruff, long black tail, makes loud calls',
        presentation: 'Here I am as a ruffed lemur: careful, I can get really loud!'
      }
    }
  }
}[LANGUE];

// Nom, description ou présentation d'une espèce dans la langue de la page, sinon le texte de lemuriens.json.
function texteEspece(l, champ) {
  var t = TEXTES.especes[l.id];
  return (t && t[champ]) || l[champ] || '';
}

// Boutons « Copier » (empreinte du fichier).
(function () {
  if (!navigator.clipboard) return;
  document.querySelectorAll('[data-copier]').forEach(function (bouton) {
    var cible = document.getElementById(bouton.dataset.copier);
    if (!cible) return;
    bouton.hidden = false;
    bouton.addEventListener('click', function () {
      navigator.clipboard.writeText(cible.textContent.trim()).then(function () {
        bouton.textContent = TEXTES.copie;
        setTimeout(function () { bouton.textContent = TEXTES.copier; }, 2000);
      });
    });
  });
})();

// Le lémurien qui se promène en bas de la page. Ses images sont dessinées par l'application elle-même
// (`lkatta site-images`) : planches horizontales de vignettes, personnage tourné vers la droite, pieds en bas.
// Il est décoratif (aria-hidden) ; tout ce qu'il permet se fait aussi au clavier depuis « Choisis ton lémurien ».
(function () {
  var DOSSIER = RACINE + 'img/lemuriens/';
  var scene = document.querySelector('.lemurien-scene');
  var el = document.getElementById('lemurien');
  var bulle = document.getElementById('bulle-lemurien');
  var annonce = document.getElementById('annonce');
  var conteneurChoix = document.getElementById('choix-lemurien');
  var colonne = document.querySelector('main');
  if (!scene || !el || !bulle || !window.fetch || !window.requestAnimationFrame) return;

  var PHRASES = TEXTES.phrases;
  var BONJOUR = TEXTES.bonjour;
  var MAX_SPONTANEES = 6;     // par visite : il parle peu
  var DODO_APRES = 90;        // secondes sans souris, clavier ni défilement

  var mouvementReduit = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)');
  function reduit() { return !!(mouvementReduit && mouvementReduit.matches); }

  function lire(cle) { try { return localStorage.getItem(cle); } catch (e) { return null; } }
  function ecrire(cle, v) { try { localStorage.setItem(cle, v); } catch (e) { /* stockage indisponible : tant pis */ } }
  function hasard(a, b) { return a + Math.random() * (b - a); }

  var donnees = null, espece = null;
  var taille = 120;
  var x = 0, dir = -1, saut = 0;
  var etat = 'assis', debutEtat = 0, finEtat = 0, cible = 0;
  var prochainClin = 0, dernierSalut = -1e9, dernierNumero = 0;
  var derniereActivite = 0, prochainePhrase = 0, spontanees = 0, ordrePhrases = [];
  var finBulle = 0, enMarche = false, dernierT = 0, maintenant = 0;
  var cacheFond = '', cachePos = '', cacheTransfo = '';

  function anim(nom) { return espece.animations[nom]; }

  // Zones où il peut marcher : les marges de la colonne de lecture si elles sont assez larges, sinon toute la largeur.
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
    var min = zs[0][0], max = zs[zs.length - 1][1];
    x = Math.min(Math.max(x, min), max);
  }

  function changer(nouvel, duree) {
    etat = nouvel;
    debutEtat = maintenant;
    finEtat = maintenant + (duree || 0);
    el.classList.toggle('dort', etat === 'dodo');
  }

  function decider() {
    if (reduit()) { changer('assis', 1e9); return; }
    // Tant qu'une bulle est affichée, il reste assis.
    if (finBulle > maintenant) { changer('assis', finBulle - maintenant + 1000); return; }
    var inactif = (maintenant - derniereActivite) / 1000;
    if (inactif > DODO_APRES && finBulle < maintenant) { changer('dodo', 1e9); return; }
    var r = Math.random();
    if (r < 0.15 && maintenant - dernierNumero > 40000) {
      var n = anim('numero');
      dernierNumero = maintenant;
      changer('numero', 2 * n.images / n.ips * 1000);
      return;
    }
    if (r < 0.7) {
      var zs = zones(), z = zoneDe(x);
      if (zs.length > 1 && Math.random() < 0.15) z = (z === zs[0]) ? zs[1] : zs[0];
      cible = hasard(z[0], z[1]);
      if (Math.abs(cible - x) < 40) { changer('assis', hasard(3000, 7000)); return; }
      dir = cible > x ? 1 : -1;
      changer('marche', 1e9);
      return;
    }
    changer('assis', hasard(4000, 9000));
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
    finBulle = maintenant + 4500 + texte.length * 40;
    // Il s'arrête dès qu'il parle, comme dans l'application.
    if (etat === 'marche' || etat === 'dodo') changer('assis', finBulle - maintenant + 1000);
    if (annoncer && annonce) annonce.textContent = texte;
    placerBulle();
  }

  function phraseSuivante() {
    if (!ordrePhrases.length) ordrePhrases = PHRASES.slice().sort(function () { return Math.random() - 0.5; });
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
    var nom = etat === 'assis' ? 'assis' : etat;
    var a = anim(nom);
    var t = (maintenant - debutEtat) / 1000;
    var image = 0;
    if (nom === 'assis') {
      // Il cligne des yeux de temps en temps (deuxième image), pas en boucle.
      if (!reduit() && maintenant >= prochainClin) {
        if (maintenant < prochainClin + 160) image = 1;
        else prochainClin = maintenant + hasard(2500, 6000);
      }
    } else if (nom === 'salut' || nom === 'numero') {
      image = Math.min(Math.floor(t * a.ips) % a.images, a.images - 1);
    } else if (a.images > 1) {
      image = Math.floor(t * a.ips) % a.images;
    }
    var fond = 'url("' + DOSSIER + a.fichier + '")|' + a.images + '|' + taille;
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
      if (Math.abs(cible - x) <= pas) { x = cible; changer('assis', hasard(4000, 10000)); }
      else x += dir * pas;
    } else if (etat === 'vol') {
      var p = Math.min((maintenant - debutEtat) / (finEtat - debutEtat), 1);
      saut = Math.sin(Math.PI * p) * taille * 0.45;
      if (p >= 1) changer('assis', hasard(2500, 5000));
    } else if (maintenant >= finEtat) {
      decider();
    }

    if (finBulle && maintenant > finBulle) { bulle.classList.remove('visible'); finBulle = 0; }
    if (!reduit() && spontanees < MAX_SPONTANEES && maintenant > prochainePhrase && etat !== 'dodo' && !finBulle) {
      spontanees++;
      prochainePhrase = maintenant + hasard(35000, 60000);
      dire(spontanees === 1 ? BONJOUR : phraseSuivante());
    }
    dessiner();
    requestAnimationFrame(tick);
  }

  function demarrer() {
    if (enMarche || scene.hidden || !espece) return;
    enMarche = true;
    dernierT = 0;
    requestAnimationFrame(tick);
  }
  function arreter() { enMarche = false; }

  function mesurer() {
    taille = el.offsetWidth || 120;
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
        ecrire('lkatta-espece', e.id);
        mesurer();
        el.classList.add('pret');
        maintenant = performance.now();
        prochainClin = maintenant + hasard(2000, 4000);
        if (reduit()) changer('assis', 1e9);
        else { dir = x > document.documentElement.clientWidth / 2 ? -1 : 1; changer('vol', 650); }
        if (presenter) dire(texteEspece(e, 'presentation'), true);
        dessiner();
        demarrer();
      };
      img.src = DOSSIER + e.animations[n].fichier;
    });
  }

  function construireChoix() {
    if (!conteneurChoix) return;
    var fs = document.createElement('fieldset');
    fs.className = 'choix';
    var legende = document.createElement('legend');
    legende.textContent = TEXTES.legende;
    fs.appendChild(legende);
    var grille = document.createElement('div');
    grille.className = 'choix-grille';
    donnees.lemuriens.forEach(function (l) {
      var label = document.createElement('label');
      label.className = 'espece';
      var input = document.createElement('input');
      input.type = 'radio';
      input.name = 'espece';
      input.value = l.id;
      input.checked = espece ? l.id === espece.id : l.id === (lire('lkatta-espece') || 'catta');
      input.addEventListener('change', function () { if (input.checked) choisir(l.id, true); });
      var apercu = document.createElement('span');
      apercu.className = 'apercu';
      apercu.setAttribute('aria-hidden', 'true');
      var a = l.animations.assis;
      apercu.style.backgroundImage = 'url("' + DOSSIER + a.fichier + '")';
      apercu.style.backgroundSize = (a.images * 72) + 'px 72px';
      var texte = document.createElement('span');
      var nom = document.createElement('span');
      nom.className = 'nom';
      nom.textContent = texteEspece(l, 'nom');
      var desc = document.createElement('span');
      desc.className = 'desc';
      var d = texteEspece(l, 'description');
      desc.textContent = d.charAt(0).toUpperCase() + d.slice(1) + '.';
      texte.appendChild(nom);
      texte.appendChild(desc);
      label.appendChild(input);
      label.appendChild(apercu);
      label.appendChild(texte);
      grille.appendChild(label);
    });
    fs.appendChild(grille);

    var actions = document.createElement('div');
    actions.className = 'choix-actions';
    var bonjour = document.createElement('button');
    bonjour.type = 'button';
    bonjour.className = 'petit-bouton';
    bonjour.textContent = TEXTES.direBonjour;
    bonjour.addEventListener('click', function () {
      if (!espece) return;
      if (scene.hidden) { caseVisible.checked = true; montrer(true); }
      maintenant = performance.now();
      derniereActivite = maintenant;
      saluer();
      dire(phraseSuivante(), true);
    });
    var labelCase = document.createElement('label');
    var caseVisible = document.createElement('input');
    caseVisible.type = 'checkbox';
    caseVisible.checked = !scene.hidden;
    caseVisible.addEventListener('change', function () { montrer(caseVisible.checked); });
    labelCase.appendChild(caseVisible);
    labelCase.appendChild(document.createTextNode(TEXTES.promener));
    actions.appendChild(bonjour);
    actions.appendChild(labelCase);
    fs.appendChild(actions);

    conteneurChoix.textContent = '';
    conteneurChoix.appendChild(fs);
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
      if (lire('lkatta-cache') === '1') scene.hidden = true;
      construireChoix();
      maintenant = derniereActivite = performance.now();
      prochainePhrase = maintenant + 5000;
      taille = el.offsetWidth || 120;
      var zs = zones();
      x = zs[zs.length - 1][1];
      choisir(lire('lkatta-espece') || 'catta', false);

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
      if (mouvementReduit && mouvementReduit.addEventListener) {
        mouvementReduit.addEventListener('change', function () { if (espece) { maintenant = performance.now(); decider(); } });
      }
    })
    .catch(function () { scene.hidden = true; });
})();
