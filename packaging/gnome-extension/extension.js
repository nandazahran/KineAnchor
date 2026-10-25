// SPDX-License-Identifier: GPL-3.0-or-later
import St from 'gi://St';
import GLib from 'gi://GLib';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class KineAnchorExtension extends Extension {
    enable() {
        this._container = new St.Widget({
            name: 'kineanchor-overlay',
            reactive: false,
            can_focus: false,
            track_hover: false,
        });
        Main.layoutManager.addTopChrome(this._container, {
            affectsInputRegion: false,
            trackFullscreen: true,
        });

        this._dots = [];
        const monitor = Main.layoutManager.primaryMonitor;
        this._container.set_position(monitor.x, monitor.y);
        this._container.set_size(monitor.width, monitor.height);

        const count = 16;
        for (let i = 0; i < count; i++) {
            const dot = new St.Widget({
                style: 'background-color: rgba(220, 250, 255, 0.75); border-radius: 6px;',
                width: 12,
                height: 12,
                reactive: false,
            });
            this._container.add_child(dot);
            this._dots.push(dot);
        }

        this._phase = 0.0;
        this._timerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 20, () => {
            this._phase += 0.02;
            const dy = Math.sin(this._phase * 2.0 * Math.PI * 0.2) * 24.0;
            const m = Main.layoutManager.primaryMonitor;
            for (let i = 0; i < this._dots.length; i++) {
                const isRight = (i % 2) === 1;
                const row = Math.floor(i / 2);
                const x = isRight ? (m.width - 48) : 36;
                const baseY = ((row + 0.5) / (this._dots.length / 2)) * m.height;
                const y = (baseY + dy + m.height) % m.height;
                this._dots[i].set_position(x, y);
            }
            return GLib.SOURCE_CONTINUE;
        });
    }

    disable() {
        if (this._timerId) {
            GLib.source_remove(this._timerId);
            this._timerId = null;
        }
        if (this._container) {
            Main.layoutManager.removeChrome(this._container);
            this._container.destroy();
            this._container = null;
        }
        this._dots = [];
    }
}
