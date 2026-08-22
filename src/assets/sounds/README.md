# Notification sounds

Four short sounds, played by `src/chime.js` for the two events the settings window's General tab
offers a choice for. They are the second thing in `src/assets/` after the app icon, and the only
audio in the tree.

| shipped as | came in as |
|---|---|
| `sound-1.mp3` | `universfield-new-notification-017-352293.mp3` |
| `sound-2.mp3` | `universfield-new-notification-036-485897.mp3` |
| `sound-3.mp3` | `universfield-new-notification-054-494259.mp3` |
| `sound-4.mp3` | `universfield-new-notification-059-494262.mp3` |

They were handed over for this task in a folder of their own, under those names; the naming is
Pixabay's for the author *Universfield*. The originals are recorded here rather than the shipped
names alone, because a bare `sound-3.mp3` is a file nobody can ever replace with confidence.

The ids in `src/sounds.js` are `sound-1` … `sound-4` and the labels are `Sound 1` … `Sound 4`:
nobody who wrote this code has heard them, and a label like `Chime` or `Bell` would be an invention
presented as a fact. Somebody who has listened can rename them in one line of `src/sounds.js` —
the file names, and therefore what `settings.json` stores, do not have to move with the labels.
