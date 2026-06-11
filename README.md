> [!WARNING]
> This project is in active development, there may be errors and bugs.

<div align="center">

<img width="200" height="200" alt="rust-dock" src="https://github.com/user-attachments/assets/073e72aa-ef28-421b-aa56-5246fbbbcf6d" />

</div>

<h1 align="center">rust-dock</h1>

<div align="center">

[![GTK4](https://img.shields.io/badge/GTK4-f5c2e7?style=for-the-badge "GTK4 - The Toolkit for Creating Graphical User Interfaces")](https://www.gtk.org/)
[![Rust](https://img.shields.io/badge/Rust-f38ba8?style=for-the-badge "Rust - Systems programming language")](https://www.rust-lang.org/)
[![Cargo](https://img.shields.io/badge/Cargo-fab387?style=for-the-badge "Cargo - The Rust package manager")](https://doc.rust-lang.org/cargo/)
[![Hyprland](https://img.shields.io/badge/Hyprland-abd6fd?style=for-the-badge "Hyprland - A dynamic tiling Wayland compositor based on wlroots that doesn't sacrifice on its looks")](https://hyprland.org/)
[![grim](https://img.shields.io/badge/grim-94e2d5?style=for-the-badge "grim - Grab images from a Wayland compositor for window-preview thumbnails")](https://git.sr.ht/~emersion/grim)

</div>

<div align="center">
 <p><i>A modern, high-performance dock for Hyprland built with Rust and GTK4.</i></p>
</div>

[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=PREVIEW)](https://git.io/typing-svg)

<table>
 <tr>
  <td width="50%"> <img width="458" height="76" alt="image" src="https://github.com/user-attachments/assets/cc51d4cf-cc65-4fbc-822c-2f6eeab1551c" />


</td>
    <td width="50%"> <img width="575" height="81" alt="image" src="https://github.com/user-attachments/assets/da69cd20-ef07-408a-9fd6-fc4039cba8f7" />


</td>
  </tr>
  <tr>
    <td width="50%"><img width="505" height="78" alt="image" src="https://github.com/user-attachments/assets/4302c793-9e7b-487c-8112-956926af41e0" />


</td>
    <td width="50%"><img width="499" height="81" alt="image" src="https://github.com/user-attachments/assets/a763b22a-6233-4a8c-9a80-33875df5fd1d" />

</td>
  </tr>
</table>

Everything syncs with pywal automatically, there are to sections the left one which, there are two sections, one on the left side where the applications are anchored and the right side where they are not anchored

<a href="https://git.io/typing-svg"><img src="https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=VIEW+OF+OPEN+WINDOWS+IN+REAL+TIME" alt="Typing SVG" /></a>

<div align="center">

<img width="678" height="300" alt="A" src="https://github.com/user-attachments/assets/d94e2966-c7f7-494b-9552-d49c753eb1fa" />

</div>

All the windows change in real time, you can see that the window plays at 30 fps

[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=DRAG+%26+DROP)](https://git.io/typing-svg)


<img width="508" height="82" alt="rec_20260611_213323" src="https://github.com/user-attachments/assets/64dbe9a0-42bd-4877-8b7d-ca903e0a411d" />


[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=INSTALL)](https://git.io/typing-svg)

For installation it is done through the commands below

```bash
git clone https://github.com/rhythmcreative/rust-dock.git
cd ~/rust-dock
./install.sh
```
> [!IMPORTANT]
> Do <b>NOT</b> run `install.sh` as sudo so that the installation is done correctly

[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=UNINSTALL)](https://git.io/typing-svg)

Run the following script
```bash
./uninstall.sh
```

[![Typing SVG](https://readme-typing-svg.herokuapp.com?font=Fira+Code&pause=1000&color=F7F7F7&vCenter=true&multiline=true&width=435&height=35&lines=COMANDS)](https://git.io/typing-svg)


```bash
❯ rust-dock -h

Options:
  -p, --position <POSITION>
          Dock position: top | bottom | left | right
  -i, --icon-size <ICON_SIZE>
          Icon size in pixels
      --padding <PADDING>
          Internal padding of the dock
      --spacing <SPACING>
          Space between application icons
      --radius <RADIUS>
          Corner radius of the dock
      --opacity <OPACITY>
          Background opacity (0.0 - 1.0)
  -o, --output <OUTPUT>
          Target a specific monitor by connector name (e.g. DP-6)
  -l, --launcher-command <LAUNCHER_COMMAND>
          Command run by the launcher button
      --launcher
          Show the launcher button
      --no-launcher
          Hide the launcher button
  -x, --exclusive-zone
          Reserve screen space so other windows don't overlap the dock (moves other windows aside)
      --no-exclusive-zone
          Don't reserve screen space
      --smart-view
          Enable smart view (auto-hide until the cursor reaches the edge)
      --style <STYLE>
          Path to an extra CSS file appended to the stylesheet
  -y, --layer <LAYER>
          Layer shell layer: overlay | top | bottom | background
  -m, --margin <MARGIN>
          Margin between the dock and the screen edge
      --system-gap
          Align the margin with Hyprland's gaps_out
      --no-system-gap
          Do not align the margin with Hyprland's gaps_out
  -h, --help
          Print help (see more with '--help')
  -V, --version
          Print version

```
<div align="center">
 <p><i>Hope you enjoy it :) </i></p>
</div>
