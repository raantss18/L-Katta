// L Katta — pont avec GNOME Shell.
//
// GNOME (Wayland) ne laisse pas une application voir les autres fenêtres. Cette extension :
// - envoie la fenêtre active (plein écran, position, application, titre) au service D-Bus
//   `org.lkatta.Maki` de L Katta, comme le script KWin sous KDE, avec la position des fenêtres
//   visibles (le maki saute de l'une à l'autre) ;
// - offre sur le bus de session, chez org.gnome.Shell, l'objet /org/lkatta/Gnome : liste des
//   fenêtres et actions (premier plan, réduire, agrandir, fermer), demandées par L Katta quand
//   l'utilisateur le lui dit.
// Rien d'autre : pas de lecture du contenu des fenêtres, pas de frappe, pas de réseau.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const INTERFACE = `<node>
  <interface name="org.lkatta.Gnome">
    <method name="Fenetres">
      <arg type="s" direction="out" name="json"/>
    </method>
    <method name="Action">
      <arg type="u" direction="in" name="id"/>
      <arg type="s" direction="in" name="action"/>
      <arg type="b" direction="out" name="ok"/>
    </method>
    <property name="Version" type="u" access="read"/>
  </interface>
</node>`;

const MAKI = 'lkatta';

function classe(w) {
    return w.get_wm_class() || w.get_gtk_application_id() || '';
}

function agrandie(w) {
    if (typeof w.is_maximized === 'function')
        return w.is_maximized();
    return w.get_maximized() === Meta.MaximizeFlags.BOTH;
}

export default class LKattaExtension extends Extension {
    enable() {
        this._objet = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
        this._objet.export(Gio.DBus.session, '/org/lkatta/Gnome');
        this._fenetre = null;
        this._liens = [];
        this._attente = 0;
        this._derniere = '';
        this._liensBureau = [
            global.display.connect('notify::focus-window', () => this._suivre()),
            global.display.connect('in-fullscreen-changed', () => this._bientot()),
            // Une fenêtre passe devant une autre, s'ouvre ou se ferme : les toits changent.
            global.display.connect('restacked', () => this._bientot()),
        ];
        this._lienBureaux = global.workspace_manager.connect('active-workspace-changed', () => this._bientot());
        // Écran branché, débranché ou échelle changée : la scène change de taille.
        this._lienEcrans = Main.layoutManager.connect('monitors-changed', () => this._bientot());
        this._suivre();
    }

    disable() {
        Main.layoutManager.disconnect(this._lienEcrans);
        global.workspace_manager.disconnect(this._lienBureaux);
        for (const id of this._liensBureau)
            global.display.disconnect(id);
        this._liensBureau = [];
        this._lacher();
        if (this._attente) {
            GLib.source_remove(this._attente);
            this._attente = 0;
        }
        this._objet.unexport();
        this._objet = null;
    }

    get Version() {
        return 3;
    }

    // Fenêtres visibles du bureau courant, de la plus basse à la plus haute : [x, y, l, h].
    _toits() {
        const bureau = global.workspace_manager.get_active_workspace();
        const l = bureau.list_windows().filter(w =>
            w.get_window_type() === Meta.WindowType.NORMAL && !w.minimized && !w.is_fullscreen() &&
            !w.is_skip_taskbar() && classe(w) !== MAKI);
        return global.display.sort_windows_by_stacking(l).map(w => {
            const r = w.get_frame_rect();
            return [r.x, r.y, r.width, r.height];
        });
    }

    _lacher() {
        if (this._fenetre) {
            for (const id of this._liens)
                this._fenetre.disconnect(id);
        }
        this._fenetre = null;
        this._liens = [];
    }

    // La fenêtre active change : on suit la nouvelle (titre, position, plein écran).
    _suivre() {
        const w = global.display.focus_window;
        // Le maki lui-même (bulle cliquée) : la fenêtre active d'avant reste la bonne.
        if (w && classe(w) === MAKI)
            return;
        this._lacher();
        if (w) {
            this._fenetre = w;
            for (const signal of ['notify::title', 'notify::fullscreen', 'position-changed', 'size-changed'])
                this._liens.push(w.connect(signal, () => this._bientot()));
            this._liens.push(w.connect('unmanaged', () => {
                this._lacher();
                this._bientot();
            }));
        }
        this._envoyer();
    }

    // Les déplacements arrivent par dizaines : un envoi toutes les 150 ms au plus.
    _bientot() {
        if (this._attente)
            return;
        this._attente = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 150, () => {
            this._attente = 0;
            this._envoyer();
            return GLib.SOURCE_REMOVE;
        });
    }

    _envoyer() {
        // sw, sh : taille de la scène (px logiques de GNOME) ; L Katta, sous Xwayland mis à
        // l'échelle, en déduit le facteur à appliquer.
        const d = {plein: false, x: 0, y: 0, w: 0, h: 0, classe: '', titre: '', ecran: '', sw: global.stage.width, sh: global.stage.height};
        const w = this._fenetre;
        if (w && w.get_window_type() !== Meta.WindowType.DESKTOP) {
            const r = w.get_frame_rect();
            Object.assign(d, {plein: w.is_fullscreen(), x: r.x, y: r.y, w: r.width, h: r.height, classe: classe(w), titre: w.get_title() || ''});
        }
        d.toits = this._toits();
        const json = JSON.stringify(d);
        if (json === this._derniere)
            return;
        this._derniere = json;
        // Pas de démarrage automatique : si L Katta ne tourne pas, rien ne se passe.
        Gio.DBus.session.call('org.lkatta.Maki', '/Maki', 'org.lkatta.Maki', 'Fenetre',
            new GLib.Variant('(s)', [json]), null, Gio.DBusCallFlags.NO_AUTO_START, 1000, null,
            (c, res) => {
                try {
                    c.call_finish(res);
                } catch (e) {
                    // L Katta absent : on renverra l'état à la prochaine occasion.
                    this._derniere = '';
                }
            });
    }

    _normales() {
        return global.display.list_all_windows().filter(w =>
            w.get_window_type() === Meta.WindowType.NORMAL && !w.is_skip_taskbar() && classe(w) !== MAKI);
    }

    Fenetres() {
        const active = global.display.focus_window;
        return JSON.stringify(this._normales().map(w => ({
            id: w.get_stable_sequence(),
            titre: w.get_title() || '',
            classe: classe(w),
            active: w === active,
            plein: w.is_fullscreen(),
            agrandie: agrandie(w),
            reduite: w.minimized,
        })));
    }

    Action(id, action) {
        const w = this._normales().find(f => f.get_stable_sequence() === id);
        if (!w)
            return false;
        const t = global.get_current_time();
        switch (action) {
        case 'activer':
            w.activate(t);
            return true;
        case 'reduire':
            w.minimize();
            return true;
        case 'agrandir':
            // GNOME 49 a retiré l'argument de maximize/unmaximize.
            if (agrandie(w)) {
                try {
                    w.unmaximize();
                } catch (e) {
                    w.unmaximize(Meta.MaximizeFlags.BOTH);
                }
            } else {
                try {
                    w.maximize();
                } catch (e) {
                    w.maximize(Meta.MaximizeFlags.BOTH);
                }
            }
            return true;
        case 'fermer':
            w.delete(t);
            return true;
        default:
            return false;
        }
    }
}
