# Pictures of kura

Both the **social preview** and the **three README screenshots** are done. What
follows is the brief they were shot to, kept because the next reshoot -- a theme
change, a layout change, a feature worth showing -- wants the same answers.

## The social preview — done

`banner.html` is the source, `social-preview.png` is it rendered. GitHub has no
API for the setting, so it is uploaded by hand at **Settings > General > Social
preview**, and that upload is the only step left.

Edit the HTML rather than the PNG; the file's own header comment has the
command that re-renders it. Since v0.109.18 it has the same shape as the other
uchmk apps' banners: 蔵, the name and the tagline on the left, and on the right
a **real screenshot**, `banner-shot.png` -- a release build with the window at
1400x1010, in `src/` with `rename.rs` under the cursor. Retake that file when
the window changes, and the banner follows.

## The three README screenshots

They are in the README, immediately after the `cargo run` line. What each one is
for, and what it had to contain:

### `screenshot-main.png` — what the program is

The three columns with a source file under the cursor and its preview alongside,
plus the outline down the right. Shot in `kura`'s own `src/`, which matters: a
reader recognises a source tree and cannot tell whether fixtures are a demo.

The frame has to include the **header** (path and counts), the **status bar**
(mode, size, date, branch), and a **preview with syntax colours**. Each answers a
question the prose cannot.

### `screenshot-keys.gif` — why it is worth using

**The one that matters.** The program's whole argument is that the keyboard is
faster, and a still frame cannot make it: it shows a layout, not a way of
working.

About fifteen seconds, looping, no mouse cursor, scaled to 1200px wide -- a
full-width GIF is one nobody waits for. **Each key is captioned** in the corner
as it is pressed (`Tab  spot panel`, `g  which-key: what can follow`, ...);
without that the reader sees things change and cannot tell which key did it.
The v0.109.18 take, in `src/`:

1. `<Tab>` for the spot panel, then `<Esc>`
2. `j` `j` `j` down the list, the preview keeping up with each row
3. a chord half-typed -- `g` on its own -- so the **which-key panel** appears
4. finish it (`gg`) so the panel resolves into the jump
5. `l` into a directory, `h` back out

Pause a second or more after each action. Recorded faster than that, nobody
can tell what happened. It was recorded with ffmpeg's `gdigrab` at 15 fps and
turned into a GIF at 10 fps with a 128-colour palette.

**Keep it under 4 MB.** A GIF that takes a moment to load is one nobody watches
to the end.

### `screenshot-preview.png` — what it does that others do not

The spot panel (`<Tab>`) over a source file: its details, the preview's line and
outline counts, the commit that last touched it, its encoding and line endings.
One frame, one idea -- if two features are on screen the reader will not know
which they were meant to notice.

Pick a file whose **last commit author** is not a machine or account name: the
panel prints the author as git has it, and on the owner's machine that can be
`MACHINE\user`. The v0.109.18 shot is `src/glob.rs` for that reason.

---

## Two things to check before committing a reshoot

**Nothing private in the frame.** A path with a real name in it, a directory of
work files, a git branch naming something internal. Easy to miss while
concentrating on the layout, permanent once pushed. The first pass of these had
a column of personal project directories down the left and was retaken inside
`src/` for exactly that reason. Look at the whole frame, not just the part being
demonstrated.

**The file sizes.** These are the first thing a reader downloads, before they
have decided they care. Run PNGs through an optimiser; they come out of a
screenshot tool two or three times larger than they need to be.
