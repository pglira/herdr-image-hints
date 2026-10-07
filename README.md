# herdr-image-hints

A [Herdr](https://herdr.dev) plugin to look at the images that tools and coding agents mention in a pane. Press one key, and a one-letter hint appears on every image path on the screen. Type a hint, and the image opens in a popup.

The popup draws the image with the Kitty graphics protocol. Herdr forwards Kitty graphics from its panes to a capable outer terminal, such as Ghostty, kitty, or WezTerm. The image shows in the terminal itself, so this also works where `xdg-open` cannot open a window: in a dev container, over SSH, or without a desktop.

## Install

```sh
herdr plugin install pglira/herdr-image-hints
```

The install step builds the binary with `cargo` when it is on `PATH`. Without `cargo`, it downloads the static binary of the release (Linux x86_64 only).

Add a key binding to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "alt+shift+y"
type = "plugin_action"
command = "pglira.herdr-image-hints.start"
description = "show an image with hints"
```

Then run `herdr server reload-config`.

## Use

1. Press the key. Every image path on the screen gets a hint. A path can be absolute (`/tmp/plot.png`), relative to the working directory of the pane (`out/fig.jpg`), or start with `~/`.
2. Type the hint. The image opens in a popup, scaled to fit and centered. The footer shows the absolute path, the size, and the position in the directory.
3. In the popup:
   - `j` (or `↓`, `→`) shows the next image in the same directory, `k` (or `↑`, `←`) the previous one.
   - `y` copies the absolute path to the clipboard (OSC 52).
   - Any other key closes the popup.

`Esc`, `q` or `Ctrl+C` closes the hints without a pick, and `?` shows the help. A path that does not exist gives a Herdr notification.

Supported formats: PNG, JPEG, GIF (first frame), WebP, BMP, TIFF, ICO, QOI, TGA, PNM, OpenEXR and Radiance HDR.

## Open an image from other programs

`herdr-image-hints open <image>` opens the same popup from any program that runs in a Herdr pane. The binary is at `target/release/herdr-image-hints` in the plugin directory:

```sh
root="$(herdr plugin list --plugin pglira.herdr-image-hints --json | jq -r '.result.plugins[0].plugin_root')"
ln -s "$root/target/release/herdr-image-hints" ~/.local/bin/herdr-image-hints
```

For example, a [yazi](https://yazi-rs.github.io) key that shows the hovered file (`keymap.toml`):

```toml
[[mgr.prepend_keymap]]
on   = "<C-y>"
run  = "shell 'herdr-image-hints open %h'"
desc = "Show the image in a herdr popup"
```

## Configuration

The plugin writes a commented `config.toml` with the defaults on its first run. Print its directory with:

```sh
herdr plugin config-dir pglira.herdr-image-hints
```

You can set the keyboard layout or the alphabet of the hints, the hint position, the extensions that make a path an image path, your own regex patterns, the size of the popup (default `85%` × `85%`), and the colors. See [`examples/config.toml`](examples/config.toml).

To see which hints a screen would get, run:

```sh
herdr pane read <pane-id> | herdr-image-hints scan
```

## Credits

The hint overlay comes from [herdr-fingers](https://github.com/nathan-poncet/herdr-fingers) by Nathan Poncet (MIT), a port of tmux-fingers. This plugin keeps its overlay and replaces its actions with the image popup.

## License

MIT
